//! 系统状态采样：每秒读取一次 CPU、内存、GPU 占用，通过 `system-stats` 事件推送给前端。

use serde::Serialize;
use std::{thread, time::Duration};
use tauri::{AppHandle, Emitter};

const GB: f64 = 1024.0 * 1024.0 * 1024.0;
/// GPU 计数器每隔多少次采样重建一次，确保新启动的进程（比如游戏本身）能被统计到
const GPU_REBUILD_EVERY: u32 = 30;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    /// 0～100
    cpu: f32,
    mem_used_gb: f64,
    mem_total_gb: f64,
    /// 0～100；读不到（比如非 Windows 或驱动不支持）时为 None
    gpu: Option<f32>,
}

pub fn spawn(app: AppHandle) {
    thread::Builder::new()
        .name("sysmon".into())
        .spawn(move || {
            let Ok(mut sys) = sysinfo::System::new() else {
                eprintln!("[sysmon] 无法初始化 sysinfo，系统状态不可用");
                return;
            };
            sys.refresh_cpu_usage(); // CPU 占用需要两次采样之间的差值，先预热一次
            let mut gpu = gpu::GpuSampler::new();
            let mut ticks: u32 = 0;

            loop {
                thread::sleep(Duration::from_secs(1));
                sys.refresh_cpu_usage();
                sys.refresh_memory();

                let stats = SystemStats {
                    cpu: sys.global_cpu_usage(),
                    mem_used_gb: sys.used_memory() as f64 / GB,
                    mem_total_gb: sys.total_memory() as f64 / GB,
                    gpu: gpu.as_mut().and_then(|g| g.sample()),
                };
                let _ = app.emit("system-stats", stats);

                ticks = ticks.wrapping_add(1);
                if ticks % GPU_REBUILD_EVERY == 0 {
                    gpu = gpu::GpuSampler::new();
                }
            }
        })
        .expect("无法启动系统状态采样线程");
}

#[cfg(windows)]
mod gpu {
    //! 用 Windows 性能计数器“GPU Engine”读取 GPU 占用，和任务管理器的算法一致：
    //! 把同一块显卡、同一种引擎（3D、视频解码、复制……）上所有进程的占用相加，取最大的那一种。

    use std::collections::HashMap;
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Performance::*;

    pub struct GpuSampler {
        query: PDH_HQUERY,
        counter: PDH_HCOUNTER,
        buf: Vec<u64>,
    }

    impl GpuSampler {
        pub fn new() -> Option<Self> {
            unsafe {
                let mut query = PDH_HQUERY::default();
                if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                    return None;
                }
                let mut counter = PDH_HCOUNTER::default();
                if PdhAddEnglishCounterW(query, w!("\\GPU Engine(*)\\Utilization Percentage"), 0, &mut counter) != 0 {
                    PdhCloseQuery(query);
                    return None;
                }
                // 第一次采集只是建立基准值
                PdhCollectQueryData(query);
                Some(Self { query, counter, buf: Vec::new() })
            }
        }

        pub fn sample(&mut self) -> Option<f32> {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return None;
                }

                let mut size = 0u32;
                let mut count = 0u32;
                let status = PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
                if status != PDH_MORE_DATA {
                    // 没有任何 GPU 引擎实例：当作空闲
                    return if status == 0 { Some(0.0) } else { None };
                }

                // 缓冲区里先是结构体数组，后面跟着实例名字符串；用 u64 保证 8 字节对齐
                self.buf.resize((size as usize).div_ceil(8), 0);
                let items_ptr = self.buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
                if PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items_ptr)) != 0 {
                    return None;
                }
                let items = std::slice::from_raw_parts(items_ptr, count as usize);

                let mut sums: HashMap<(String, String), f64> = HashMap::new();
                for item in items {
                    let st = item.FmtValue.CStatus;
                    if st != PDH_CSTATUS_VALID_DATA && st != PDH_CSTATUS_NEW_DATA {
                        continue;
                    }
                    let name = item.szName.to_string().unwrap_or_default();
                    *sums.entry(parse_instance(&name)).or_default() += item.FmtValue.Anonymous.doubleValue;
                }

                let max = sums.values().copied().fold(0.0_f64, f64::max);
                Some(max.clamp(0.0, 100.0) as f32)
            }
        }
    }

    impl Drop for GpuSampler {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }

    /// 实例名形如 `pid_1234_luid_0x00000000_0x0000C3A1_phys_0_eng_0_engtype_3D`，
    /// 取出显卡标识（luid）和引擎类型（engtype）。
    fn parse_instance(name: &str) -> (String, String) {
        let luid = name
            .split_once("luid_")
            .and_then(|(_, rest)| rest.split_once("_phys").map(|(l, _)| l))
            .unwrap_or("")
            .to_string();
        let engtype = name.rsplit_once("engtype_").map(|(_, t)| t).unwrap_or("").to_string();
        (luid, engtype)
    }

    #[cfg(test)]
    mod tests {
        use super::parse_instance;

        #[test]
        fn parses_instance_name() {
            let (luid, eng) = parse_instance("pid_1234_luid_0x00000000_0x0000C3A1_phys_0_eng_0_engtype_3D");
            assert_eq!(luid, "0x00000000_0x0000C3A1");
            assert_eq!(eng, "3D");
        }

        /// 在真机上读一次 GPU 占用：cargo test --lib -- --ignored --nocapture
        #[test]
        #[ignore]
        fn live_sample() {
            let mut s = super::GpuSampler::new().expect("无法创建 GPU 计数器");
            for _ in 0..3 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                println!("GPU = {:?}", s.sample());
            }
        }

        #[test]
        fn parses_engine_type_with_spaces() {
            let (_, eng) = parse_instance("pid_8_luid_0x0_0x1_phys_0_eng_3_engtype_VideoDecode");
            assert_eq!(eng, "VideoDecode");
        }
    }
}

#[cfg(not(windows))]
mod gpu {
    /// 其他系统暂不支持读取 GPU 占用
    pub struct GpuSampler;

    impl GpuSampler {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn sample(&mut self) -> Option<f32> {
            None
        }
    }
}
