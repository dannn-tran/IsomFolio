namespace IsomFolio.Addons.Faces;

public static class Program
{
    public static void Main()
    {
        var outbox = new MessageOutbox(Console.Out);
        outbox.SendHello(["cluster_faces"]);
        
        outbox.SendLog(LogLevel.Info, "loading face models…");
        IRequestHandler requestHandler;
        try
        {
            var requestHandlerFactory =
                new RequestHandlerFactory(outbox, new DbscanConfigFactory(outbox), new ModelDownloader(outbox));
            requestHandler = requestHandlerFactory.Create();
        }
        catch (Exception e)
        {
            outbox.SendLog(LogLevel.Error, $"model init failed: {e.Message}");
            return;
        }
        
        outbox.SendLog(LogLevel.Info, "ready");

        while (MessageInbox.TryGetNext(out var msg))
        {
            if (msg is ClusterFacesRequest req)
            {
                try
                {
                    var result = requestHandler.Handle(req);
                    outbox.SendResponse(req.Id, result);
                }
                catch (Exception ex)
                {
                    outbox.SendError(req.Id, ex.Message);
                }
            }
            else
            {
                outbox.SendError(msg.Id, $"unknown request: {msg}");
            }
        }
    }
}
