FROM rust:latest AS chef

RUN update-ca-certificates

RUN cargo install cargo-chef

# planner
FROM chef AS planner

COPY . .

RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS cook

COPY --from=planner recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json


FROM cook AS buildah

# Create appuser
ENV USER=app
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /buildah

COPY ./ .

RUN cargo build --release

FROM gcr.io/distroless/cc

# Import from builder.
COPY --from=buildah /etc/passwd /etc/passwd
COPY --from=buildah /etc/group /etc/group

WORKDIR /app

# Copy our build
COPY --from=buildah /buildah/target/release/geranium ./

# Use an unprivileged user.
USER app:app

CMD ["/app/geranium"]
