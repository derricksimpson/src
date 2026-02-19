using Src.Models;

namespace Src.Output;

public static class YamlOutputWriter
{
    public static void Write(OutputEnvelope result, TextWriter output)
    {
        if (result.Meta is { } meta)
            WriteMeta(output, meta);

        if (result.Error is not null)
            WriteScalar(output, "error", result.Error, 0);

        if (result.Tree is not null)
            WriteTree(output, result.Tree, isRoot: true);

        if (result.Files is { Count: > 0 })
            WriteFiles(output, result.Files);
    }

    static void WriteMeta(TextWriter w, MetaInfo meta)
    {
        w.Write("meta:\n");
        if (meta.ElapsedMs != 0)
            w.Write($"  elapsedMs: {meta.ElapsedMs}\n");
        if (meta.Timeout)
            w.Write("  timeout: true\n");
        if (meta.FilesScanned != 0)
            w.Write($"  filesScanned: {meta.FilesScanned}\n");
        if (meta.FilesMatched != 0)
            w.Write($"  filesMatched: {meta.FilesMatched}\n");
    }

    static void WriteFiles(TextWriter w, List<FileEntry> files)
    {
        w.Write("files:\n");
        foreach (var file in files)
        {
            w.Write("- path: ");
            WriteInlineString(w, file.Path);
            w.Write('\n');

            if (file.Error is not null)
            {
                w.Write("  error: ");
                WriteInlineString(w, file.Error);
                w.Write('\n');
            }

            if (file.Contents is not null)
                WriteBlockScalar(w, "contents", file.Contents, 2);

            if (file.Chunks is { Count: > 0 })
            {
                w.Write("  chunks:\n");
                foreach (var chunk in file.Chunks)
                {
                    w.Write($"  - startLine: {chunk.StartLine}\n");
                    w.Write($"    endLine: {chunk.EndLine}\n");
                    WriteBlockScalar(w, "content", chunk.Content, 4);
                }
            }
        }
    }

    static void WriteTree(TextWriter w, ScanResult node, bool isRoot)
    {
        if (isRoot)
        {
            w.Write("tree:\n");
            WriteTreeNode(w, node, 2);
        }
        else
        {
            WriteTreeNode(w, node, 2);
        }
    }

    static void WriteTreeNode(TextWriter w, ScanResult node, int indent)
    {
        var pad = new string(' ', indent);
        w.Write($"{pad}name: ");
        WriteInlineString(w, node.Name);
        w.Write('\n');

        if (node.Files is { Count: > 0 })
        {
            w.Write($"{pad}files:\n");
            foreach (var file in node.Files)
            {
                w.Write($"{pad}- ");
                WriteInlineString(w, file);
                w.Write('\n');
            }
        }

        if (node.Children is { Count: > 0 })
        {
            w.Write($"{pad}children:\n");
            foreach (var child in node.Children)
            {
                w.Write($"{pad}- ");
                // first key on the same line as the dash
                w.Write("name: ");
                WriteInlineString(w, child.Name);
                w.Write('\n');

                var childIndent = indent + 2;
                var childPad = new string(' ', childIndent);

                if (child.Files is { Count: > 0 })
                {
                    w.Write($"{childPad}files:\n");
                    foreach (var file in child.Files)
                    {
                        w.Write($"{childPad}- ");
                        WriteInlineString(w, file);
                        w.Write('\n');
                    }
                }

                if (child.Children is { Count: > 0 })
                {
                    w.Write($"{childPad}children:\n");
                    foreach (var grandchild in child.Children)
                    {
                        w.Write($"{childPad}- ");
                        WriteTreeNodeInline(w, grandchild, childIndent + 2);
                    }
                }
            }
        }
    }

    static void WriteTreeNodeInline(TextWriter w, ScanResult node, int indent)
    {
        var pad = new string(' ', indent);

        w.Write("name: ");
        WriteInlineString(w, node.Name);
        w.Write('\n');

        if (node.Files is { Count: > 0 })
        {
            w.Write($"{pad}files:\n");
            foreach (var file in node.Files)
            {
                w.Write($"{pad}- ");
                WriteInlineString(w, file);
                w.Write('\n');
            }
        }

        if (node.Children is { Count: > 0 })
        {
            w.Write($"{pad}children:\n");
            foreach (var child in node.Children)
            {
                w.Write($"{pad}- ");
                WriteTreeNodeInline(w, child, indent + 2);
            }
        }
    }

    static void WriteScalar(TextWriter w, string key, string value, int indent)
    {
        var pad = indent > 0 ? new string(' ', indent) : "";
        w.Write($"{pad}{key}: ");
        WriteInlineString(w, value);
        w.Write('\n');
    }

    static void WriteBlockScalar(TextWriter w, string key, string content, int indent)
    {
        var pad = new string(' ', indent);
        w.Write($"{pad}{key}: |\n");
        foreach (var line in content.AsSpan().EnumerateLines())
        {
            if (line.IsEmpty)
                w.Write('\n');
            else
            {
                w.Write(pad);
                w.Write("  ");
#if NET9_0_OR_GREATER
                w.Write(line);
#else
                w.Write(line.ToString());
#endif
                w.Write('\n');
            }
        }
    }

    static void WriteInlineString(TextWriter w, string value)
    {
        if (value.Length == 0)
        {
            w.Write("''");
            return;
        }

        if (NeedsQuoting(value))
        {
            w.Write('"');
            foreach (var c in value)
            {
                switch (c)
                {
                    case '"': w.Write("\\\""); break;
                    case '\\': w.Write("\\\\"); break;
                    case '\n': w.Write("\\n"); break;
                    case '\r': w.Write("\\r"); break;
                    case '\t': w.Write("\\t"); break;
                    default: w.Write(c); break;
                }
            }
            w.Write('"');
        }
        else
        {
            w.Write(value);
        }
    }

    static bool NeedsQuoting(string value)
    {
        if (value.Length == 0) return true;

        var first = value[0];
        if (first is '-' or '[' or ']' or '{' or '}' or '\'' or '"' or '!' or '&' or '*' or '|' or '>' or '%' or '@' or '`' or ',' or '?' or '#')
            return true;

        if (value is "true" or "false" or "null" or "True" or "False" or "Null" or "TRUE" or "FALSE" or "NULL" or "yes" or "no" or "Yes" or "No" or "YES" or "NO" or "on" or "off" or "On" or "Off" or "ON" or "OFF")
            return true;

        foreach (var c in value)
        {
            if (c is ':' or '#' or '\n' or '\r')
                return true;
        }

        return false;
    }
}
