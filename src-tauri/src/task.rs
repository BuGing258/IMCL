//! 安装任务的上下文：取消标记、进度计数，以及定时把进度推给前端。

use anyhow::{bail, Result};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub struct TaskCtx {
    pub id: String,
    pub cancel: Arc<AtomicBool>,
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub bytes: AtomicU64,
    stage: Mutex<(String, u32, u32)>,
    finished: AtomicBool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub task_id: String,
    pub stage: String,
    pub stage_index: u32,
    pub stage_count: u32,
    pub done: u64,
    pub total: u64,
    /// 字节/秒
    pub speed: u64,
}

impl TaskCtx {
    pub fn new(id: String, cancel: Arc<AtomicBool>, stage_count: u32) -> Arc<Self> {
        Arc::new(Self {
            id,
            cancel,
            done: AtomicU64::new(0),
            total: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            stage: Mutex::new((String::from("准备中"), 0, stage_count)),
            finished: AtomicBool::new(false),
        })
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn check_cancel(&self) -> Result<()> {
        if self.is_cancelled() {
            bail!("已取消");
        }
        Ok(())
    }

    /// 进入下一个阶段，计数清零
    pub fn stage(&self, name: &str) {
        let mut s = self.stage.lock().unwrap();
        s.0 = name.to_string();
        s.1 += 1;
        if s.1 > s.2 {
            s.2 = s.1;
        }
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
    }

    /// 当前阶段：(名称, 序号, 总数)
    pub fn current_stage(&self) -> (String, u32, u32) {
        self.stage.lock().unwrap().clone()
    }

    pub fn finish(&self) {
        self.finished.store(true, Ordering::Relaxed);
    }

    fn snapshot(&self, speed: u64) -> ProgressEvent {
        let s = self.stage.lock().unwrap();
        ProgressEvent {
            task_id: self.id.clone(),
            stage: s.0.clone(),
            stage_index: s.1,
            stage_count: s.2,
            done: self.done.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            speed,
        }
    }

    /// 每 300ms 推送一次进度事件（`install-progress` / `launch-progress`），直到 finish()
    pub fn spawn_reporter(self: &Arc<Self>, app: AppHandle, event: &'static str) {
        let ctx = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut last_bytes = 0u64;
            let mut last_t = Instant::now();
            let mut speed = 0f64;
            while !ctx.finished.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(300)).await;
                let b = ctx.bytes.load(Ordering::Relaxed);
                let dt = last_t.elapsed().as_secs_f64().max(0.001);
                let inst = (b.saturating_sub(last_bytes)) as f64 / dt;
                speed = speed * 0.6 + inst * 0.4; // 平滑一下，避免数字乱跳
                last_bytes = b;
                last_t = Instant::now();
                let _ = app.emit(event, ctx.snapshot(speed as u64));
            }
        });
    }
}
