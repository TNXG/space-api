# Build Stage
FROM rust:alpine AS builder

WORKDIR /app

# 安装构建依赖
RUN apk add --no-cache musl-dev nasm

# 复制依赖定义文件，构建缓存
COPY Cargo.toml Cargo.lock ./

# 创建空的src/main.rs避免cargo报错
RUN mkdir src && echo "fn main() {}" > src/main.rs

# 预先构建依赖
RUN cargo build --release

# 复制全部源码（包括src/templates）
COPY ./src ./src

# 重新构建正式二进制
RUN cargo build --release

# 运行时镜像
FROM alpine:latest

WORKDIR /app

RUN apk add --no-cache ca-certificates tzdata

COPY --from=builder /app/target/release/space-api-rs .

COPY --from=builder /app/src/templates ./src/templates

EXPOSE 3000

ENV CONFIG_PATH=/app/config.toml
ENV ROCKET_ADDRESS=0.0.0.0

CMD ["./space-api-rs"]
