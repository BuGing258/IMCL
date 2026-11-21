//! 内存分配：手动分配的上限是电脑总内存减去 2 GB 余量；自动分配按版本里的 Mod 数量来定

use serde::Serialize;
use std::path::Path;

/// 留给系统和其他程序的内存
pub const RESERVE_MB: u32 = 2048;
const STEP: u32 = 512;
const MIN_MB: u32 = 1024;

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPlan {
    /// 电脑总内存
    pub total_mb: u32,
    /// 手动分配最多能给多少
    pub max_mb: u32,
    /// 自动分配会给多少
    pub auto_mb: u32,
}

fn total_mb() -> u32 {
    // 读不到就当 8 GB
    let Ok(mut sys) = sysinfo::System::new() else { return 8192 };
    sys.refresh_memory();
    (sys.total_memory() / 1024 / 1024) as u32
}

/// 版本目录里有多少个启用的 Mod；没有 mods 文件夹就是 0
fn mod_count(game_dir: &Path, version: &str) -> usize {
    let Ok(dir) = crate::game::installed::run_dir(game_dir, version) else { return 0 };
    std::fs::read_dir(dir.join("mods"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".jar"))
                .count()
        })
        .unwrap_or(0)
}

/// 自动分配：原版 2 GB；有 Mod 从 3 GB 起，每个 Mod 多给一点，最多 12 GB；不超过手动上限
pub fn auto_mb(max: u32, mods: usize) -> u32 {
    let need = if mods == 0 { 2048 } else { (3072 + mods as u32 * 40).min(12288) };
    let need = need.div_ceil(STEP) * STEP;
    need.min(max).max(MIN_MB)
}

pub fn max_mb(total: u32) -> u32 {
    (total.saturating_sub(RESERVE_MB) / STEP * STEP).max(MIN_MB)
}

/// `version` 为空时按原版算
pub fn plan(game_dir: &Path, version: Option<&str>) -> MemoryPlan {
    let total = total_mb();
    let max = max_mb(total);
    let mods = version.map(|v| mod_count(game_dir, v)).unwrap_or(0);
    MemoryPlan { total_mb: total, max_mb: max, auto_mb: auto_mb(max, mods) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits() {
        assert_eq!(max_mb(16 * 1024), 14 * 1024);
        assert_eq!(max_mb(1500), MIN_MB);
        assert_eq!(auto_mb(14336, 0), 2048);
        assert_eq!(auto_mb(14336, 10), 3584);
        assert_eq!(auto_mb(14336, 300), 12288);
        assert_eq!(auto_mb(6144, 300), 6144);
    }
}
