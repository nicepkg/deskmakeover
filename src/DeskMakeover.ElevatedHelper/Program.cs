if (args.Length == 0)
{
    Console.WriteLine("DeskMakeover elevated helper.");
    Console.WriteLine("No operation requested.");
    return 0;
}

var command = args[0].Trim().ToLowerInvariant();

return command switch
{
    "version" => WriteVersion(),
    _ => WriteUnknown(command)
};

static int WriteVersion()
{
    Console.WriteLine("DeskMakeover.ElevatedHelper 0.1.0");
    return 0;
}

static int WriteUnknown(string command)
{
    Console.Error.WriteLine($"Unsupported helper operation: {command}");
    return 2;
}

