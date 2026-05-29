using System.Threading.Channels;

namespace IsomFolio.Extensions.Sdk.Tests;

public class MessageReaderTests
{
    private static readonly IExtensionLogger NullLogger = new NullExtensionLogger();

    private static async Task<List<InboundMessage>> CollectAsync(string input, CancellationToken ct = default)
    {
        var msgs = new List<InboundMessage>();
        await foreach (var msg in MessageReader.ReadAllAsync(new StringReader(input), NullLogger, ct))
            msgs.Add(msg);
        return msgs;
    }

    [Fact]
    public async Task ParsesHandshakeRequest()
    {
        var msgs = await CollectAsync("{\"id\":1,\"method\":\"handshake\"}\n", TestContext.Current.CancellationToken);
        var req = Assert.IsType<HandshakeRequest>(Assert.Single(msgs));
        Assert.Equal(1UL, req.Id);
    }

    [Fact]
    public async Task ParsesPingRequest()
    {
        var msgs = await CollectAsync("{\"id\":2,\"method\":\"ping\"}\n", TestContext.Current.CancellationToken);
        var req = Assert.IsType<PingRequest>(Assert.Single(msgs));
        Assert.Equal(2UL, req.Id);
    }

    [Fact]
    public async Task ParsesClusterFacesRequest()
    {
        var msgs = await CollectAsync(
            "{\"id\":1,\"method\":\"cluster_faces\",\"params\":{\"files\":[],\"force_full\":true}}\n",
            TestContext.Current.CancellationToken);

        var req = Assert.IsType<ClusterFacesRequest>(Assert.Single(msgs));
        Assert.Equal(1UL, req.Id);
        Assert.True(req.Params.ForceFull);
        Assert.Empty(req.Params.Files);
    }

    [Fact]
    public async Task ReturnsFalseOnEof()
    {
        var msgs = await CollectAsync("", TestContext.Current.CancellationToken);
        Assert.Empty(msgs);
    }

    [Fact]
    public async Task SkipsBlankLines()
    {
        var msgs = await CollectAsync(
            "\n\n{\"id\":2,\"method\":\"cluster_faces\",\"params\":{\"files\":[],\"force_full\":false}}\n",
            TestContext.Current.CancellationToken);
        Assert.Equal(2UL, Assert.Single(msgs).Id);
    }

    [Fact]
    public async Task DeserializesClusterFacesFileParams()
    {
        var json = """
            {"id":1,"method":"cluster_faces","params":{"files":[{"file_id":"abc","image_path":"/tmp/a.jpg","file_mtime":123}],"force_full":false}}
            """;
        var msgs = await CollectAsync(json + "\n", TestContext.Current.CancellationToken);

        var req = Assert.IsType<ClusterFacesRequest>(Assert.Single(msgs));
        Assert.Single(req.Params.Files);
        Assert.Equal("abc", req.Params.Files[0].FileId);
        Assert.Equal("/tmp/a.jpg", req.Params.Files[0].ImagePath);
        Assert.Equal(123L, req.Params.Files[0].FileMtime);
    }

    [Fact]
    public async Task ParsesMultipleMessages()
    {
        var input = "{\"id\":1,\"method\":\"ping\"}\n{\"id\":2,\"method\":\"handshake\"}\n";
        var msgs = await CollectAsync(input, TestContext.Current.CancellationToken);
        Assert.Equal(2, msgs.Count);
        Assert.IsType<PingRequest>(msgs[0]);
        Assert.IsType<HandshakeRequest>(msgs[1]);
    }

    [Fact]
    public async Task StopsOnCancellation()
    {
        var ct = TestContext.Current.CancellationToken;
        var channel = Channel.CreateUnbounded<string>();
        var reader = channel.Reader.AsTextReader();
        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);

        var msgs = new List<InboundMessage>();
        var readTask = Task.Run(async () =>
        {
            await foreach (var msg in MessageReader.ReadAllAsync(reader, NullLogger, cts.Token))
                msgs.Add(msg);
        }, ct);

        await channel.Writer.WriteAsync("{\"id\":1,\"method\":\"ping\"}\n", ct);
        await Task.Delay(50, ct);
        cts.Cancel();
        await readTask;

        Assert.Single(msgs);
    }
}

file static class ChannelExtensions
{
    public static TextReader AsTextReader(this ChannelReader<string> channel) =>
        new ChannelTextReader(channel);

    private sealed class ChannelTextReader(ChannelReader<string> channel) : TextReader
    {
        private string _buffer = "";
        private int _pos;

        public override async ValueTask<string?> ReadLineAsync(CancellationToken ct)
        {
            while (true)
            {
                var nl = _buffer.IndexOf('\n', _pos);
                if (nl >= 0)
                {
                    var line = _buffer[_pos..nl];
                    _pos = nl + 1;
                    return line;
                }

                if (!await channel.WaitToReadAsync(ct)) return null;
                while (channel.TryRead(out var chunk))
                    _buffer = _buffer[_pos..] + chunk;
                _pos = 0;
            }
        }
    }
}

file sealed class NullExtensionLogger : IExtensionLogger
{
    public ValueTask LogAsync(LogLevel level, string message) => ValueTask.CompletedTask;
}
