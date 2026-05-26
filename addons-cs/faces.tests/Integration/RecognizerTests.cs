namespace IsomFolio.Addons.Faces.Tests.Integration;

public class RecognizerTests(ModelFixture models) : IClassFixture<ModelFixture>
{
    [Fact]
    public void Recognizer_LoadsWithoutError()
    {
        using var recognizer = new FaceRecognizer(models.RecPath);
    }
}