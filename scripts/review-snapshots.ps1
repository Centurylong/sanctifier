#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Review and approve insta snapshot diffs for Sanctifier detectors.

.DESCRIPTION
    This script provides a streamlined workflow for reviewing snapshot changes
    in Sanctifier detector tests. It shows pending diffs, allows interactive
    review, and helps maintain transparency in detector changes.

.PARAMETER TestOnly
    Run snapshot tests without reviewing changes.

.PARAMETER Review
    Run interactive review of pending snapshot changes.

.PARAMETER AcceptAll
    Accept all pending snapshot changes (use with caution).

.PARAMETER RejectAll
    Reject all pending snapshot changes.

.PARAMETER ListPending
    List all pending snapshot files without taking action.

.PARAMETER DetectorsOnly
    Only review detector snapshots, not gallery snapshots.

.EXAMPLE
    .\scripts\review-snapshots.ps1 -TestOnly
    Run snapshot tests to see what has changed.

.EXAMPLE
    .\scripts\review-snapshots.ps1 -Review
    Interactively review pending snapshot changes.

.EXAMPLE
    .\scripts\review-snapshots.ps1 -ListPending
    List all pending snapshot files.

.EXAMPLE
    .\scripts\review-snapshots.ps1 -Review -DetectorsOnly
    Review only detector snapshot changes.
#>

param(
    [switch]$TestOnly,
    [switch]$Review,
    [switch]$AcceptAll,
    [switch]$RejectAll,
    [switch]$ListPending,
    [switch]$DetectorsOnly
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$rootDir = Split-Path -Parent $scriptDir
$snapshotDir = Join-Path $rootDir "tooling\sanctifier-core\tests\snapshots"

function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

function Test-Snapshots {
    Write-ColorOutput Cyan "Running snapshot tests..."
    Push-Location $rootDir
    
    $extraArgs = @()
    if ($DetectorsOnly) {
        $extraArgs += "--", "detector_snapshots"
    }
    
    cargo insta test -p sanctifier-core --all-features @extraArgs
    $testResult = $LASTEXITCODE
    
    Pop-Location
    
    if ($testResult -eq 0) {
        Write-ColorOutput Green "✓ All snapshots match - no changes detected."
    } else {
        Write-ColorOutput Yellow "⚠ Snapshot changes detected. Review pending diffs."
    }
    
    return $testResult
}

function Get-PendingSnapshots {
    $pendingFiles = @()
    
    if (Test-Path $snapshotDir) {
        $pendingFiles = Get-ChildItem -Path $snapshotDir -Filter "*.snap.new" -Recurse -ErrorAction SilentlyContinue
    }
    
    if ($DetectorsOnly) {
        $pendingFiles = $pendingFiles | Where-Object { $_.Name -like "detector_snapshots__*" }
    }
    
    return $pendingFiles
}

function Show-PendingSnapshots {
    $pendingFiles = Get-PendingSnapshots
    
    if ($pendingFiles.Count -eq 0) {
        Write-ColorOutput Green "No pending snapshot files found."
        return
    }
    
    Write-ColorOutput Yellow "Pending snapshot files ($($pendingFiles.Count)):"
    Write-ColorOutput Cyan "----------------------------------------"
    
    foreach ($file in $pendingFiles) {
        $relativePath = $file.FullName.Substring($rootDir.Length + 1)
        $originalName = $file.Name -replace '\.snap\.new$', '.snap'
        Write-Output "  • $relativePath"
        Write-Output "    Original: $originalName"
        
        # Show file size comparison
        $originalPath = Join-Path $file.DirectoryName $originalName
        if (Test-Path $originalPath) {
            $newSize = (Get-Item $file.FullName).Length
            $oldSize = (Get-Item $originalPath).Length
            $diff = $newSize - $oldSize
            $change = if ($diff -gt 0) { "+$diff bytes" } elseif ($diff -lt 0) { "$diff bytes" } else { "no size change" }
            Write-Output "    Size change: $change"
        }
        
        Write-Output ""
    }
}

function Show-Diff($file) {
    $originalName = $file.Name -replace '\.snap\.new$', '.snap'
    $originalPath = Join-Path $file.DirectoryName $originalName
    
    if (-not (Test-Path $originalPath)) {
        Write-ColorOutput Red "Original snapshot not found: $originalPath"
        Write-ColorOutput Cyan "New snapshot content:"
        Get-Content $file.FullName
        return
    }
    
    Write-ColorOutput Cyan "Diff for $($file.Name):"
    Write-ColorOutput Cyan "================================"
    
    # Simple diff using Compare-Object
    $oldContent = Get-Content $originalPath
    $newContent = Get-Content $file.FullName
    
    $changes = Compare-Object $oldContent $newContent
    
    if ($changes.Count -eq 0) {
        Write-ColorOutput Green "No content differences detected."
    } else {
        foreach ($change in $changes) {
            $line = $change.InputObject
            if ($change.SideIndicator -eq "<=") {
                Write-ColorOutput Red "- $line"
            } else {
                Write-ColorOutput Green "+ $line"
            }
        }
    }
}

function Invoke-Review {
    Write-ColorOutput Cyan "Starting interactive snapshot review..."
    Write-ColorOutput Cyan "=========================================`n"
    
    $pendingFiles = Get-PendingSnapshots
    
    if ($pendingFiles.Count -eq 0) {
        Write-ColorOutput Green "No pending snapshot files to review."
        Write-ColorOutput Yellow "Run with -TestOnly first to generate pending diffs."
        return
    }
    
    foreach ($file in $pendingFiles) {
        Write-ColorOutput Yellow "`nReviewing: $($file.Name)"
        Write-ColorOutput Cyan "----------------------------------------"
        Show-Diff $file
        
        $choice = ""
        while ($choice -notin @("a", "r", "s", "q")) {
            Write-ColorOutput Cyan "`nChoose action:"
            Write-Output "  [a] Accept this change"
            Write-Output "  [r] Reject this change"
            Write-Output "  [s] Skip for now"
            Write-Output "  [q] Quit review"
            $choice = Read-Host "Your choice"
        }
        
        switch ($choice) {
            "a" {
                $originalName = $file.Name -replace '\.snap\.new$', '.snap'
                $originalPath = Join-Path $file.DirectoryName $originalName
                Move-Item -Force $file.FullName $originalPath
                Write-ColorOutput Green "✓ Accepted: $($file.Name)"
            }
            "r" {
                Remove-Item $file.FullName
                Write-ColorOutput Red "✗ Rejected: $($file.Name)"
            }
            "s" {
                Write-ColorOutput Yellow "⊘ Skipped: $($file.Name)"
            }
            "q" {
                Write-ColorOutput Yellow "Review stopped by user."
                return
            }
        }
    }
    
    Write-ColorOutput Green "`n✓ Review complete."
}

function Invoke-AcceptAll {
    Write-ColorOutput Yellow "⚠ WARNING: Accepting all pending snapshot changes without review."
    $confirm = Read-Host "Are you sure you want to continue? (yes/no)"
    
    if ($confirm -ne "yes") {
        Write-ColorOutput Cyan "Operation cancelled."
        return
    }
    
    Push-Location $rootDir
    cargo insta accept
    Pop-Location
    
    Write-ColorOutput Green "✓ All pending snapshots accepted."
}

function Invoke-RejectAll {
    Write-ColorOutput Yellow "⚠ WARNING: Rejecting all pending snapshot changes."
    $confirm = Read-Host "Are you sure you want to continue? (yes/no)"
    
    if ($confirm -ne "yes") {
        Write-ColorOutput Cyan "Operation cancelled."
        return
    }
    
    Push-Location $rootDir
    cargo insta reject
    Pop-Location
    
    Write-ColorOutput Green "✓ All pending snapshots rejected."
}

# Main execution
if ($TestOnly) {
    Test-Snapshots
} elseif ($ListPending) {
    Show-PendingSnapshots
} elseif ($Review) {
    Test-Snapshots
    Invoke-Review
} elseif ($AcceptAll) {
    Invoke-AcceptAll
} elseif ($RejectAll) {
    Invoke-RejectAll
} else {
    Write-ColorOutput Cyan "Sanctifier Snapshot Review Tool"
    Write-ColorOutput Cyan "=============================="
    Write-Output ""
    Write-Output "Usage:"
    Write-Output "  -TestOnly      Run snapshot tests to detect changes"
    Write-Output "  -ListPending   List pending snapshot files"
    Write-Output "  -Review        Interactively review pending changes"
    Write-Output "  -AcceptAll     Accept all pending changes (caution!)"
    Write-Output "  -RejectAll     Reject all pending changes"
    Write-Output "  -DetectorsOnly  Filter to detector snapshots only"
    Write-Output ""
    Write-Output "Examples:"
    Write-Output "  .\scripts\review-snapshots.ps1 -TestOnly"
    Write-Output "  .\scripts\review-snapshots.ps1 -ListPending"
    Write-Output "  .\scripts\review-snapshots.ps1 -Review"
    Write-Output "  .\scripts\review-snapshots.ps1 -Review -DetectorsOnly"
}
