using System.Runtime.InteropServices;
using System.Text.Json;
using DeskMakeover.IconRendering;
using Microsoft.Win32;

namespace DeskMakeover.ElevatedHelper;

/// <summary>
/// The only privileged operations this helper exposes (spec 01 Elevated.Helper):
/// applying and restoring the global shortcut-overlay badge. No arbitrary
/// commands, no scripts. The helper persists the ORIGINAL registry state under
/// %ProgramData%\DeskMakeover so restore never depends on the caller.
/// </summary>
public static class OverlayCommands
{
    private const string ShellIconsKeyPath = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Shell Icons";
    private const string OverlayValueName = "29";
    private const string AbsentMarker = "__absent__";
    private const int ShcneAssocChanged = 0x08000000;

    [DllImport("shell32.dll")]
    private static extern void SHChangeNotify(int wEventId, uint uFlags, nint dwItem1, nint dwItem2);

    private static string DataDirectory =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "DeskMakeover");

    private static string StatePath => Path.Combine(DataDirectory, "overlay-state.json");

    public static int Apply(string style)
    {
        if (style is not ("refined" or "transparent"))
        {
            Console.Error.WriteLine($"Unsupported overlay style: {style}");
            return 2;
        }

        Directory.CreateDirectory(DataDirectory);

        var factory = new OverlayBadgeIconFactory();
        var fileName = style == "refined" ? "refined-mark.ico" : "clear.ico";
        var icoPath = Path.Combine(DataDirectory, fileName);
        File.WriteAllBytes(icoPath, style == "refined"
            ? factory.CreateRefinedMarkIco()
            : factory.CreateTransparentIco());

        using var key = Registry.LocalMachine.CreateSubKey(ShellIconsKeyPath)
            ?? throw new InvalidOperationException("Could not open the Shell Icons registry key.");

        // Capture the pre-DeskMakeover state exactly once; re-applying a different
        // style must not overwrite the true original.
        if (!File.Exists(StatePath))
        {
            var original = key.GetValue(OverlayValueName) as string ?? AbsentMarker;
            File.WriteAllText(StatePath, JsonSerializer.Serialize(new OverlayState(original)));
        }

        key.SetValue(OverlayValueName, $"{icoPath},0", RegistryValueKind.String);
        SHChangeNotify(ShcneAssocChanged, 0, 0, 0);
        Console.WriteLine($"overlay applied: {style}");
        return 0;
    }

    public static int Restore()
    {
        if (!File.Exists(StatePath))
        {
            Console.WriteLine("overlay untouched: nothing to restore");
            return 0;
        }

        var state = JsonSerializer.Deserialize<OverlayState>(File.ReadAllText(StatePath))
            ?? throw new InvalidOperationException("Overlay state file is unreadable.");

        using var key = Registry.LocalMachine.CreateSubKey(ShellIconsKeyPath)
            ?? throw new InvalidOperationException("Could not open the Shell Icons registry key.");

        if (state.Original == AbsentMarker)
        {
            key.DeleteValue(OverlayValueName, throwOnMissingValue: false);
        }
        else
        {
            key.SetValue(OverlayValueName, state.Original, RegistryValueKind.String);
        }

        File.Delete(StatePath);
        SHChangeNotify(ShcneAssocChanged, 0, 0, 0);
        Console.WriteLine("overlay restored");
        return 0;
    }

    private sealed record OverlayState(string Original);
}
