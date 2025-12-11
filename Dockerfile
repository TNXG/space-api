# 这是一个纯运行时镜像，不需要 rust 环境
FROM alpine:latest

WORKDIR /app

# 直接从当前目录（构建上下文）复制二进制文件
# 注意：文件名需要和你在 Action 中重命名的一致
COPY space-api-rs .

# 复制资源文件
COPY src/templates ./src/templates

EXPOSE 8000

ENV CONFIG_PATH=/app/config.toml
ENV ROCKET_ADDRESS=0.0.0.0

CMD ["./space-api-rs"]