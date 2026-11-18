# Hersir's Start menu shortcut for Scoop: Start Menu\Programs\Scoop Apps\Hersir.lnk, which starts hersir.exe in the app's
# folder (Scoop's current folder) and carries Hersir's AppUserModelID, dev.hersir.app. Windows shows a desktop app's
# notifications only for an ID that a Start menu shortcut has, and Scoop's own shortcuts can't carry one.
# The manifest's post_install runs it, with the app's folder:
#   & "$bucketsdir\$bucket\scripts\hersir-shortcut.ps1" -Dir $dir -Global:([bool]$global)
# and its pre_uninstall removes the shortcut, also on an update, before the new version's post_install makes it again.
# -Global: the shortcut goes into the Start menu of all users, for a global install (scoop install --global).
# Without the C# (a locked-down PowerShell) the shortcut has no app ID, and it says so; without any shortcut, the install
# still finishes. The C# is install.ps1's. scripts/packages.sh in Yotech-AI/hersir publishes this file from
# packaging/scoop/scripts/hersir-shortcut.ps1 there: changes to it go there.
[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Dir,
  [switch]$Global
)
Set-StrictMode -Version 2
$ErrorActionPreference = 'Stop'

$exe = Join-Path $Dir 'hersir.exe'
$startMenu = [Environment]::GetFolderPath($(if ($Global) { 'CommonStartMenu' } else { 'StartMenu' }))
$folder = Join-Path $startMenu 'Programs\Scoop Apps'
$link = Join-Path $folder 'Hersir.lnk'
$description = 'Clients, projects and tasks, worked on by local AI agents'

# WScript.Shell can't set an AppUserModelID, so this C# sets it through IPropertyStore (PKEY_AppUserModel_ID).
$shortcutCode = @'
using System;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;

namespace HersirScoop
{
    [ComImport, Guid("00021401-0000-0000-C000-000000000046")]
    class CShellLink { }

    [ComImport, Guid("000214F9-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IShellLinkW
    {
        void GetPath(IntPtr pszFile, int cch, IntPtr pfd, uint fFlags);
        void GetIDList(out IntPtr ppidl);
        void SetIDList(IntPtr pidl);
        void GetDescription(IntPtr pszName, int cch);
        void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string pszName);
        void GetWorkingDirectory(IntPtr pszDir, int cch);
        void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string pszDir);
        void GetArguments(IntPtr pszArgs, int cch);
        void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string pszArgs);
        void GetHotkey(out ushort pwHotkey);
        void SetHotkey(ushort wHotkey);
        void GetShowCmd(out int piShowCmd);
        void SetShowCmd(int iShowCmd);
        void GetIconLocation(IntPtr pszIconPath, int cch, out int piIcon);
        void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string pszIconPath, int iIcon);
        void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string pszPathRel, uint dwReserved);
        void Resolve(IntPtr hwnd, uint fFlags);
        void SetPath([MarshalAs(UnmanagedType.LPWStr)] string pszFile);
    }

    [StructLayout(LayoutKind.Sequential, Pack = 4)]
    struct PropertyKey
    {
        public Guid FormatId;
        public uint PropertyId;
    }

    [StructLayout(LayoutKind.Explicit, Size = 24)]
    struct PropVariant
    {
        [FieldOffset(0)] public ushort VarType;
        [FieldOffset(8)] public IntPtr Pointer;
    }

    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPropertyStore
    {
        void GetCount(out uint cProps);
        void GetAt(uint iProp, out PropertyKey pkey);
        void GetValue(ref PropertyKey key, out PropVariant pv);
        void SetValue(ref PropertyKey key, ref PropVariant pv);
        void Commit();
    }

    public static class Shortcut
    {
        // Saves a shortcut at lnkPath that starts target, with the AppUserModelID appId (PKEY_AppUserModel_ID).
        public static void Save(string lnkPath, string target, string workingDir, string description, string appId)
        {
            IShellLinkW link = (IShellLinkW)new CShellLink();
            link.SetPath(target);
            link.SetWorkingDirectory(workingDir);
            link.SetDescription(description);
            link.SetIconLocation(target, 0);
            PropertyKey key = new PropertyKey();
            key.FormatId = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3");
            key.PropertyId = 5;
            PropVariant value = new PropVariant();
            value.VarType = 31; // VT_LPWSTR
            value.Pointer = Marshal.StringToCoTaskMemUni(appId);
            try
            {
                IPropertyStore store = (IPropertyStore)link;
                store.SetValue(ref key, ref value);
                store.Commit();
            }
            finally
            {
                Marshal.FreeCoTaskMem(value.Pointer);
            }
            ((IPersistFile)link).Save(lnkPath, true);
            Marshal.ReleaseComObject(link);
        }
    }
}
'@

try {
  New-Item -ItemType Directory -Force -Path $folder | Out-Null
  if (-not ('HersirScoop.Shortcut' -as [type])) { Add-Type -TypeDefinition $shortcutCode }
  [HersirScoop.Shortcut]::Save($link, $exe, $Dir, $description, 'dev.hersir.app')
  Write-Host "Made the Start menu shortcut Hersir, with the app ID dev.hersir.app: $link"
} catch {
  # Without the C#, a plain shortcut: Hersir starts from the Start menu, without notifications.
  $why = $_.Exception.Message
  try {
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($link)
    $shortcut.TargetPath = $exe
    $shortcut.WorkingDirectory = $Dir
    $shortcut.Description = $description
    $shortcut.Save()
    Write-Host "Note: the Start menu shortcut Hersir has no app ID ($why), so Windows may not show Hersir's notifications."
  } catch {
    Write-Host "Could not make the Start menu shortcut Hersir ($why). Start Hersir with: hersir"
  }
}
