using IsomFolio.Addons.Sdk;

namespace IsomFolio.Addons.Faces.Tests;

public class FacesWorkerTests
{
    private static ClusterFacesRequest MakeRequest(ulong id) =>
        new(id, new ClusterFacesRequestParams([], ForceFull: false));

    private sealed class FakeHandler(Func<ClusterFacesRequest, CancellationToken, Task<ClusterResult>> handle)
        : IRequestHandler
    {
        public Task<ClusterResult> HandleAsync(ClusterFacesRequest request, CancellationToken ct = default) =>
            handle(request, ct);
        public void Dispose() { }
    }

    [Fact]
    public async Task DisposeAsync_CompletesAfterTasksFinishNormally()
    {
        using var writer = new MessageWriter(TextWriter.Null);
        var handler = new FakeHandler((_, _) => Task.FromResult(new ClusterResult([], [])));
        var worker = new FacesWorker(writer, Task.FromResult<IRequestHandler>(handler));

        using var cts = CancellationTokenSource.CreateLinkedTokenSource(TestContext.Current.CancellationToken);
        worker.Enqueue(MakeRequest(1), cts.Token);
        worker.Enqueue(MakeRequest(2), cts.Token);

        await worker.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5), TestContext.Current.CancellationToken);
    }

    [Fact]
    public async Task DisposeAsync_CompletesAfterCancellation_WhenTaskIsInFlight()
    {
        var ct = TestContext.Current.CancellationToken;
        using var writer = new MessageWriter(TextWriter.Null);

        var taskStarted = new TaskCompletionSource();
        var handler = new FakeHandler(async (_, token) =>
        {
            taskStarted.TrySetResult();
            await Task.Delay(Timeout.Infinite, token);
            return new ClusterResult([], []);
        });

        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        var worker = new FacesWorker(writer, Task.FromResult<IRequestHandler>(handler));
        worker.Enqueue(MakeRequest(1), cts.Token);

        await taskStarted.Task.WaitAsync(ct);
        cts.Cancel();

        await worker.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5), ct);
    }

    [Fact]
    public async Task DisposeAsync_CompletesAfterCancellation_WithMultiplePendingTasks()
    {
        var ct = TestContext.Current.CancellationToken;
        using var writer = new MessageWriter(TextWriter.Null);

        var firstStarted = new TaskCompletionSource();
        var handler = new FakeHandler(async (req, token) =>
        {
            if (req.Id == 1) firstStarted.TrySetResult();
            await Task.Delay(Timeout.Infinite, token);
            return new ClusterResult([], []);
        });

        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        var worker = new FacesWorker(writer, Task.FromResult<IRequestHandler>(handler));
        worker.Enqueue(MakeRequest(1), cts.Token);
        worker.Enqueue(MakeRequest(2), cts.Token);
        worker.Enqueue(MakeRequest(3), cts.Token);

        await firstStarted.Task.WaitAsync(ct);
        cts.Cancel();

        await worker.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5), ct);
    }

    [Fact]
    public async Task DisposeAsync_WithFaultedHandlerTask_Completes()
    {
        var ct = TestContext.Current.CancellationToken;
        using var writer = new MessageWriter(TextWriter.Null);

        var faultedTask = Task.FromException<IRequestHandler>(new InvalidOperationException("load failed"));
        var worker = new FacesWorker(writer, faultedTask);
        worker.Enqueue(MakeRequest(1), ct);

        await worker.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(5), ct);
    }

    [Fact]
    public async Task Enqueue_TasksProcessedSequentially()
    {
        var ct = TestContext.Current.CancellationToken;
        using var writer = new MessageWriter(TextWriter.Null);

        var activeCount = 0;
        var concurrencyExceeded = false;
        var handler = new FakeHandler(async (_, token) =>
        {
            if (Interlocked.Increment(ref activeCount) > 1) concurrencyExceeded = true;
            await Task.Delay(20, token);
            Interlocked.Decrement(ref activeCount);
            return new ClusterResult([], []);
        });

        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        var worker = new FacesWorker(writer, Task.FromResult<IRequestHandler>(handler));
        worker.Enqueue(MakeRequest(1), cts.Token);
        worker.Enqueue(MakeRequest(2), cts.Token);
        worker.Enqueue(MakeRequest(3), cts.Token);

        await worker.DisposeAsync();

        Assert.False(concurrencyExceeded);
    }
}
