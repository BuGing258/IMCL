@echo off
chcp 65001 >nul
title IMCL (dev)
cd /d "%~dp0"

rem 第一次运行先装依赖
if not exist "node_modules" (
  echo 第一次运行，正在安装依赖...
  call npm install || goto :fail
)

rem 1420 端口被占用时开发服务器起不来
netstat -ano | findstr /r /c:":1420 .*LISTENING" >nul && (
  echo 1420 端口已被占用：可能已经开着一个启动器了，先把它关掉再运行。
  goto :fail
)

echo 正在启动 IMCL（第一次编译要等几分钟）...
call npm run tauri dev || goto :fail
exit /b 0

:fail
echo.
echo 启动失败，看看上面的错误信息。
pause
exit /b 1
