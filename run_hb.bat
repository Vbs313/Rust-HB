@echo off
chcp 65001 >nul
title Hearthbuddy Rust Edition

echo ========================================
echo  Hearthbuddy Rust Edition — 启动脚本
echo ========================================
echo.
echo  构建模式: release
echo  日志输出: 控制台 (Ctrl+C 停止)
echo.
echo  确保 Hearthstone 已启动
echo  确保 BepInEx IPC 插件已就绪
echo.
echo ========================================

:: 切换到项目目录
cd /d "%~dp0"

:: 检查 release 二进制是否存在，不存在则构建
if not exist "target\release\hb-app.exe" (
    echo [INFO] 首次启动，正在构建...
    cargo build --release -p hb-app
    if %errorlevel% neq 0 (
        echo [ERROR] 构建失败，请检查错误信息
        pause
        exit /b 1
    )
)

echo [INFO] 启动 Rust-HB...
echo [INFO] 按 Ctrl+C 优雅停止
echo.

:: 运行
target\release\hb-app.exe

:: 退出处理
echo.
echo Bot 已停止
pause
