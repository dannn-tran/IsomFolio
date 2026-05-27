namespace IsomFolio.Addons.Faces;

public interface IRequestHandlerFactory
{
    IRequestHandler Create();
}

public class RequestHandlerFactory(IMessageOutbox outbox, IDbscanConfigFactory configFactory,
    IModelDownloader modelDownloader) : IRequestHandlerFactory
{
    public IRequestHandler Create()
    {
        var modelsDir = Environment.GetEnvironmentVariable("ISOMFOLIO_MODELS_DIR") ?? ".";
        
        var (detPath, recPath) = modelDownloader.EnsureModelsDownloaded(modelsDir);
        var detector = new FaceDetector(detPath);
        var recognizer = new FaceRecognizer(recPath);
        
        var stateDbPath = Path.Combine(modelsDir, "faces", "state.db");
        Directory.CreateDirectory(Path.GetDirectoryName(stateDbPath)!);
        var cache = new EmbeddingCache(stateDbPath);

        return new RequestHandler(configFactory.Create(), outbox, cache, detector, recognizer);
    }
}