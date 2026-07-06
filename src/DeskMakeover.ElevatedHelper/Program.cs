using DeskMakeover.ElevatedHelper;

if (args.Length == 0)
{
    Console.WriteLine("DeskMakeover elevated helper.");
    Console.WriteLine("No operation requested.");
    return 0;
}

var command = args[0].Trim().ToLowerInvariant();

try
{
    return command switch
    {
        "version" => WriteVersion(),
        "apply-overlay" => OverlayCommands.Apply(GetOption(args, "--style") ?? "refined", GetOption(args, "--file")),
        "restore-overlay" => OverlayCommands.Restore(),
        _ => WriteUnknown(command)
    };
}
catch (Exception ex)
{
    Console.Error.WriteLine($"Helper operation failed: {ex.Message}");
    return 3;
}

static string? GetOption(string[] args, string name)
{
    for (var i = 1; i < args.Length - 1; i++)
    {
        if (args[i].Equals(name, StringComparison.OrdinalIgnoreCase))
        {
            return args[i + 1].Trim().ToLowerInvariant();
        }
    }

    return null;
}

static int WriteVersion()
{
    Console.WriteLine("DeskMakeover.ElevatedHelper 0.9.0");
    return 0;
}

static int WriteUnknown(string command)
{
    Console.Error.WriteLine($"Unsupported helper operation: {command}");
    return 2;
}
