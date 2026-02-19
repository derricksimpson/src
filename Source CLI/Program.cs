using Src.Commands;

var cts = new CancellationTokenSource();
Console.CancelKeyPress += (_, e) => { e.Cancel = true; cts.Cancel(); };

string root = Environment.CurrentDirectory;
List<string> globs = [];
string? find = null;
int pad = 0;
int? timeout = null;
List<string> excludes = [];
bool noDefaults = false;
bool isRegex = false;

for (int i = 0; i < args.Length; i++)
{
    switch (args[i])
    {
        case "--root" or "-d":
            if (++i >= args.Length) { Console.Error.WriteLine("Missing value for --root"); return 1; }
            root = args[i];
            break;
        case "--r":
            if (++i >= args.Length) { Console.Error.WriteLine("Missing value for --r"); return 1; }
            globs.Add(args[i]);
            break;
        case "--f":
            if (++i >= args.Length) { Console.Error.WriteLine("Missing value for --f"); return 1; }
            find = args[i];
            break;
        case "--pad":
            if (++i >= args.Length) { Console.Error.WriteLine("Missing value for --pad"); return 1; }
            if (!int.TryParse(args[i], out pad)) { Console.Error.WriteLine($"Invalid integer for --pad: {args[i]}"); return 1; }
            break;
        case "--timeout":
            if (++i >= args.Length) { Console.Error.WriteLine("Missing value for --timeout"); return 1; }
            if (!int.TryParse(args[i], out var t)) { Console.Error.WriteLine($"Invalid integer for --timeout: {args[i]}"); return 1; }
            timeout = t;
            break;
        case "--exclude":
            if (++i >= args.Length) { Console.Error.WriteLine("Missing value for --exclude"); return 1; }
            excludes.Add(args[i]);
            break;
        case "--no-defaults":
            noDefaults = true;
            break;
        case "--regex":
            isRegex = true;
            break;
        case "--help" or "-h" or "-?":
            PrintHelp();
            return 0;
        case "--version":
            Console.WriteLine("0.1.0");
            return 0;
        default:
            Console.Error.WriteLine($"Unknown option: {args[i]}");
            Console.Error.WriteLine("Run 'src --help' for usage information.");
            return 1;
    }
}

if (!string.IsNullOrEmpty(find))
{
    return await SearchCommand.ExecuteAsync(
        root, globs.ToArray(), find, isRegex, pad, excludes.ToArray(), noDefaults, timeout, cts.Token);
}
else if (globs.Count > 0)
{
    return await ScanCommand.ExecuteFileListing(
        root, globs.ToArray(), excludes.ToArray(), noDefaults, timeout, cts.Token);
}
else
{
    return await ScanCommand.ExecuteDirectoryHierarchy(
        root, excludes.ToArray(), noDefaults, timeout, cts.Token);
}

static void PrintHelp()
{
    Console.WriteLine("""
        src — fast source code interrogation tool

        Usage:
          src [options]

        Modes:
          (default)      Show directory hierarchy containing source files
          --r <glob>     List files matching glob patterns (repeatable)
          --f <pattern>  Search file contents for a pattern

        Options:
          --root, -d <path>   Root directory (default: current directory)
          --r <glob>          File glob pattern (repeatable, e.g. --r *.ts --r *.cs)
          --f <pattern>       Search pattern (use | for OR, e.g. Payment|Invoice)
          --pad <n>           Context lines before/after each match (default: 0)
          --timeout <secs>    Max execution time in seconds
          --exclude <name>    Additional exclusions (repeatable)
          --no-defaults       Disable built-in exclusions (node_modules, .git, etc.)
          --regex             Treat --f pattern as a regular expression
          --help, -h          Show this help
          --version           Show version

        Examples:
          src                              Show directory tree
          src --r *.cs                     List all C# files
          src --r *.ts --f "import"        Search TypeScript files for imports
          src --f "TODO|FIXME" --pad 2     Find TODOs with 2 lines of context
          src -d /path/to/project          Scan a specific directory
        """);
}
