# Faces inference engine for linux/amd64.
#
# Used on Intel macOS, where ONNX Runtime 1.26.0 ships no native osx-x64
# library — the engine runs in this Linux x64 container instead. Build context
# is `extensions-cs/` (so the Sdk project is available); see
# scripts/build-faces-docker.sh.
#
# Self-contained, non-AOT: the build toolchain stays light and image size is a
# local-only concern (the image is never distributed).

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
WORKDIR /src
COPY Sdk/ Sdk/
COPY Faces/ Faces/
RUN dotnet publish Faces/Faces.csproj -c Release -r linux-x64 --self-contained \
    -p:PublishAot=false -p:PublishTrimmed=false -o /app/publish

FROM mcr.microsoft.com/dotnet/runtime-deps:10.0 AS runtime
# libgomp1: required by the ONNX Runtime CPU provider on Linux.
RUN apt-get update \
    && apt-get install -y --no-install-recommends libgomp1 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/publish/ ./
# Model cache — bind-mounted by the host so downloads survive container restarts.
VOLUME ["/models"]
EXPOSE 45876
ENTRYPOINT ["/app/faces"]
CMD ["--port", "45876", "--data-dir", "/models", "--host", "0.0.0.0"]
