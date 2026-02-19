namespace Src.Models;

public sealed class FileChunk
{
    public int StartLine { get; set; }
    public int EndLine { get; set; }
    public string Content { get; set; } = string.Empty;
}
