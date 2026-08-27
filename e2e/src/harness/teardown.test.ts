import { createServer, type Server, type Socket } from "node:net";

import { describe, expect, it } from "vitest";

import {
  aggregateErrors,
  createIdempotentStop,
  waitForPortsClosed,
} from "./teardown.js";

describe("harness teardown", () => {
  it("keeps a wedged listener open when TCP connects but HTTP never answers", async () => {
    const server = await listeningServer((socket) => socket.pause());
    const port = server.addressPort;
    try {
      await expect(waitForPortsClosed([port], 150)).rejects.toThrow(
        `WebDriver ports remain open after 150ms: ${port}`,
      );
    } finally {
      await closeServer(server);
    }
  });

  it("accepts a refused connection as proof that the listener closed", async () => {
    const server = await listeningServer();
    const port = server.addressPort;
    await closeServer(server);

    await expect(waitForPortsClosed([port], 500)).resolves.toBeUndefined();
  });

  it("shares one stop promise across concurrent calls", async () => {
    let calls = 0;
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const stop = createIdempotentStop(async () => {
      calls += 1;
      await gate;
    });

    const first = stop();
    const second = stop();
    const third = stop();
    expect(second).toBe(first);
    expect(third).toBe(first);
    await Promise.resolve();
    expect(calls).toBe(1);
    release();
    await Promise.all([first, second, third]);
  });

  it("puts the original diagnostic first when cleanup also fails", () => {
    const startup = new Error("session startup failed");
    const cleanup = new Error("listener cleanup failed");
    const combined = aggregateErrors(startup, cleanup);

    expect(combined).toBeInstanceOf(AggregateError);
    expect(combined.errors[0]).toBe(startup);
    expect(combined.errors[1]).toBe(cleanup);
    expect(combined.message).toMatch(/^session startup failed\n/);
  });
});

async function listeningServer(
  connection: (socket: Socket) => void = () => {},
): Promise<{ server: Server; addressPort: number; connections: Set<Socket> }> {
  const connections = new Set<Socket>();
  const server = createServer();
  server.on("connection", (socket) => {
    connections.add(socket);
    socket.once("close", () => connections.delete(socket));
    connection(socket);
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolve());
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("test server did not expose a TCP port");
  }
  return { server, addressPort: address.port, connections };
}

async function closeServer({
  server,
  connections,
}: {
  server: Server;
  connections: Set<Socket>;
}): Promise<void> {
  for (const socket of connections) socket.destroy();
  await new Promise<void>((resolve, reject) => {
    server.close((error) => {
      if (
        error &&
        (error as NodeJS.ErrnoException).code !== "ERR_SERVER_NOT_RUNNING"
      ) {
        reject(error);
        return;
      }
      resolve();
    });
  });
}
