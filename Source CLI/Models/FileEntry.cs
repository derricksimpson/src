namespace Src.Models;

public sealed class FileEntry
{
    public string Path { get; set; } = string.Empty;
    public string? Contents { get; set; }
    public string? Error { get; set; }
    public List<FileChunk>? Chunks { get; set; }
}
