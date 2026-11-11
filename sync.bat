@echo off
title One-Click Sync Folder
setlocal enabledelayedexpansion

echo ========================================
echo Starting folder sync...
echo ========================================

:: 1. Check Git repository
if not exist ".git" (
    echo Error: Current directory is not a Git repository!
    echo Please run git init and configure remote repository first.
    pause
    exit /b 1
)

:: 2. Detect current branch (compatible with main / master / others)
for /f "delims=" %%b in ('git rev-parse --abbrev-ref HEAD 2^>nul') do set "branch=%%b"
if "!branch!"=="" (
    echo Error: Unable to detect current Git branch.
    echo Please make sure you have at least one commit in this repository.
    pause
    exit /b 1
)
if /i "!branch!"=="HEAD" (
    echo Error: Detached HEAD state detected.
    echo Please checkout a branch first, e.g. git checkout main
    pause
    exit /b 1
)
echo Current branch: !branch!
echo.

:: 3. Generate default commit timestamp message
for /f "tokens=1-3 delims=/- " %%a in ('date /t') do (
    set date_str=%%a-%%b-%%c
)
for /f "tokens=1-2 delims=:." %%a in ("%time%") do (
    set "hour=%%a"
    set "min=%%b"
)
set "hour=%hour: =%"
set "time_str=%hour%:%min%"
set "default_msg=Sync on %date_str% %time_str%"
set "commit_msg=%default_msg%"

echo Default commit message: %default_msg%
echo.
echo Operation Tip:
echo Press Enter / Y to customize message
echo Press N to use default message directly
echo Auto customize after 40 seconds idle
echo.

:: Create temporary VBS popup (all English prompt, Enter = Yes/Y)
echo set WshShell = WScript.CreateObject("WScript.Shell") >_tmp_ask.vbs
echo ret = WshShell.Popup("Customize commit message?" ^&vbCrLf^&"Enter=Yes(Y)  N=No(Default)",40,"Select Mode",4+32) >>_tmp_ask.vbs
echo if ret = -1 or ret =6 then WScript.Echo "Y" else WScript.Echo "N" >>_tmp_ask.vbs

:: Run VBS and get selection
for /f "delims=" %%r in ('cscript //nologo _tmp_ask.vbs') do set opt=%%r
del /f /q _tmp_ask.vbs >nul 2>&1

if "!opt!"=="Y" (
    echo.
    set /p "user_msg=Input commit message: "
    if "!user_msg!"=="" (
        echo Empty input, fallback to default message
        set commit_msg=!default_msg!
    ) else (
        set commit_msg=!user_msg!
    )
) else (
    echo Use default commit message directly
    goto :do_commit
)

:do_commit
echo.
echo Using commit message: !commit_msg!
echo.

:: Git add
echo Adding all changes...
git add .
if %errorlevel% neq 0 (
    echo git add failed, please check file permissions or Git status.
    pause
    exit /b %errorlevel%
)

:: Git commit
echo Committing to local repository...
git commit -m "!commit_msg!"
set commit_err=%errorlevel%
if %commit_err% neq 0 (
    if %commit_err% equ 1 (
        echo No changes to commit, skipping commit step.
    ) else (
        echo git commit failed, please check error messages.
        pause
        exit /b %commit_err%
    )
)

:: Git pull rebase
echo Pulling latest from remote (branch: !branch!)...
git pull --rebase origin !branch!
if %errorlevel% neq 0 (
    echo ========================================
    echo Conflict detected during pull!
    echo Please resolve conflicts manually, then run git rebase --continue
    echo or use git rebase --abort to cancel this operation.
    echo ========================================
    pause
    exit /b %errorlevel%
)

:: Git push
echo Pushing to remote (branch: !branch!)...
git push origin !branch!
if %errorlevel% neq 0 (
    echo git push failed, please check network or permissions.
    pause
    exit /b %errorlevel%
)

echo ========================================
echo Sync completed!
echo Branch: !branch!
echo Commit message: !commit_msg!
echo ========================================
pause
endlocal