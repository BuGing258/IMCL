//! Maven 坐标工具：`group:artifact:version[:classifier][@ext]`

/// 坐标对应的相对路径（使用 `/` 分隔）
pub fn path(coord: &str) -> Option<String> {
    let (coord, ext) = coord.split_once('@').unwrap_or((coord, "jar"));
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 || parts[..3].iter().any(|p| p.is_empty()) {
        return None;
    }
    let (g, a, v) = (parts[0], parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(c) => format!("{a}-{v}-{c}.{ext}"),
        None => format!("{a}-{v}.{ext}"),
    };
    Some(format!("{}/{a}/{v}/{file}", g.replace('.', "/")))
}

/// 用于去重的键：同一个 group:artifact(:classifier) 只保留一个版本
pub fn dedupe_key(coord: &str) -> String {
    let coord = coord.split('@').next().unwrap_or(coord);
    let parts: Vec<&str> = coord.split(':').collect();
    match parts.len() {
        0..=2 => coord.to_string(),
        3 => format!("{}:{}", parts[0], parts[1]),
        _ => format!("{}:{}:{}", parts[0], parts[1], parts[3]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(path("org.ow2.asm:asm:9.6").unwrap(), "org/ow2/asm/asm/9.6/asm-9.6.jar");
        assert_eq!(
            path("org.lwjgl:lwjgl:3.3.1:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
        );
        assert_eq!(
            path("de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412@zip").unwrap(),
            "de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412.zip"
        );
        assert!(path("bad:coord").is_none());
    }

    #[test]
    fn keys() {
        assert_eq!(dedupe_key("org.ow2.asm:asm:9.6"), "org.ow2.asm:asm");
        assert_eq!(dedupe_key("org.lwjgl:lwjgl:3.3.1:natives-windows"), "org.lwjgl:lwjgl:natives-windows");
    }
}
