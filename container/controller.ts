import { Container } from "@cloudflare/containers";

const QUEUE_MESSAGE_VERSION = 1;
const JOB_ID_PATTERN = /^[0-9a-f]{32}$/i;
const INTERNAL_API_PREFIX = "/internal-api";

type AudioJobMessage = {
  version: number;
  job_id: string;
};

type QueueMessage<T> = {
  body: T;
  ack(): void;
};

type QueueBatch<T> = {
  messages: QueueMessage<T>[];
};

type SecretStoreBinding = {
  get(): Promise<string | undefined>;
};

type Env = {
  AUDIO_PROCESSOR: DurableObjectNamespace<AudioProcessorContainer>;
  DANTALIAN_API: Fetcher;
  DANTALIAN_PROCESSOR_TOKEN?: string;
  PROCESSOR_API_BASE_URL?: string;
  WASABI_ACCESS_KEY_ID?: string;
  WASABI_SECRET_ACCESS_KEY?: string;
  WASABI_BUCKET?: string;
  DANTALIAN_BUCKET?: string;
  WASABI_ENDPOINT?: string;
  WASABI_REGION?: string;
  WASABI_ACCESS_KEY_ID_STORE?: SecretStoreBinding;
  WASABI_SECRET_ACCESS_KEY_STORE?: SecretStoreBinding;
  WASABI_REGION_STORE?: SecretStoreBinding;
  WASABI_ENDPOINT_STORE?: SecretStoreBinding;
  WASABI_BUCKET_STORE?: SecretStoreBinding;
};

export class AudioProcessorContainer extends Container<Env> {
  sleepAfter = "5m";
  enableInternet = true;
  envVars = {};

  override onStart(): void {
    console.log(JSON.stringify({ event: "audio_processor.container_started" }));
  }

  override onStop(): void {
    console.log(JSON.stringify({ event: "audio_processor.container_stopped" }));
  }

  override onError(error: unknown): void {
    console.error(
      JSON.stringify({
        event: "audio_processor.container_error",
        error_class: error instanceof Error ? error.name : "unknown",
      }),
    );
  }
}

export default {
  async queue(batch: QueueBatch<AudioJobMessage>, env: Env): Promise<void> {
    for (const message of batch.messages) {
      const jobId = parseJobId(message.body);
      const processorToken = env.DANTALIAN_PROCESSOR_TOKEN?.trim();
      if (!processorToken) {
        throw new Error("DANTALIAN_PROCESSOR_TOKEN is not configured");
      }
      const processorBaseUrl = env.PROCESSOR_API_BASE_URL?.trim();
      if (!processorBaseUrl) {
        throw new Error("PROCESSOR_API_BASE_URL is not configured");
      }
      const [accessKeyId, secretAccessKey, endpoint, region, bucket] = await Promise.all([
        requiredSecret(
          env.WASABI_ACCESS_KEY_ID_STORE,
          env.WASABI_ACCESS_KEY_ID,
          "WASABI_ACCESS_KEY_ID",
        ),
        requiredSecret(
          env.WASABI_SECRET_ACCESS_KEY_STORE,
          env.WASABI_SECRET_ACCESS_KEY,
          "WASABI_SECRET_ACCESS_KEY",
        ),
        requiredSecret(
          env.WASABI_ENDPOINT_STORE,
          env.WASABI_ENDPOINT,
          "WASABI_ENDPOINT",
        ),
        requiredSecret(env.WASABI_REGION_STORE, env.WASABI_REGION, "WASABI_REGION"),
        requiredSecret(
          env.WASABI_BUCKET_STORE,
          env.WASABI_BUCKET ?? env.DANTALIAN_BUCKET,
          "DANTALIAN_BUCKET",
        ),
      ]);
      const container = env.AUDIO_PROCESSOR.getByName(`audio-${jobId}`);
      console.log(JSON.stringify({ event: "audio_job.container_start_requested", job_id: jobId }));
      try {
        await container.start({
          envVars: {
            DANTALIAN_AUDIO_JOB_ID: jobId,
            DANTALIAN_PROCESSOR_ID: `container-${jobId}`,
            DANTALIAN_PROCESSOR_ONCE: "1",
            DANTALIAN_WORKER_BASE_URL: processorBaseUrl,
            DANTALIAN_API_TOKEN: processorToken,
            WASABI_ACCESS_KEY_ID: accessKeyId,
            WASABI_SECRET_ACCESS_KEY: secretAccessKey,
            WASABI_BUCKET: bucket,
            WASABI_ENDPOINT: endpoint,
            WASABI_REGION: region,
          },
        });
      } catch (error) {
        console.error(
          JSON.stringify({
            event: "audio_job.container_start_failed",
            job_id: jobId,
            error_class: error instanceof Error ? error.name : "unknown",
          }),
        );
        throw error;
      }
      console.log(JSON.stringify({ event: "audio_job.container_start_accepted", job_id: jobId }));

      // Queue retry covers failures before the container accepted the job.
      // The processor owns the D1 lease and reports completion independently.
      message.ack();
    }
  },

  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/health") {
      return Response.json({ ok: true, service: "dantalian-audio-controller" });
    }
    if (!url.pathname.startsWith(`${INTERNAL_API_PREFIX}/`)) {
      return new Response("Not Found", { status: 404 });
    }

    const processorToken = env.DANTALIAN_PROCESSOR_TOKEN?.trim();
    if (!processorToken) {
      return Response.json(
        {
          error: "processor authentication is not configured",
          code: "processor_authentication_not_configured",
        },
        { status: 500 },
      );
    }
    const authorization = request.headers.get("authorization");
    if (!authorization || !constantTimeBearerEqual(authorization, processorToken)) {
      return new Response("Unauthorized", {
        status: 401,
        headers: { "www-authenticate": "Bearer" },
      });
    }

    const targetPath = url.pathname.slice(INTERNAL_API_PREFIX.length) || "/";
    const headers = new Headers(request.headers);
    headers.set("authorization", `Bearer ${processorToken}`);
    const body = request.method === "GET" || request.method === "HEAD" ? undefined : request.body;
    const target = new Request(`https://dantalian-api${targetPath}${url.search}`, {
      method: request.method,
      headers,
      body,
      redirect: "manual",
    });
    return env.DANTALIAN_API.fetch(target);
  },
};

function parseJobId(message: AudioJobMessage): string {
  if (
    message.version !== QUEUE_MESSAGE_VERSION ||
    typeof message.job_id !== "string" ||
    !JOB_ID_PATTERN.test(message.job_id)
  ) {
    throw new Error("invalid audio queue message");
  }
  return message.job_id.toLowerCase();
}

function constantTimeBearerEqual(value: string, expected: string): boolean {
  const prefix = "Bearer ";
  if (!value.startsWith(prefix)) {
    return false;
  }
  const supplied = new TextEncoder().encode(value.slice(prefix.length));
  const wanted = new TextEncoder().encode(expected);
  let difference = supplied.length ^ wanted.length;
  const length = Math.max(supplied.length, wanted.length);
  for (let index = 0; index < length; index += 1) {
    difference |= (supplied[index] ?? 0) ^ (wanted[index] ?? 0);
  }
  return difference === 0;
}

async function requiredSecret(
  binding: SecretStoreBinding | undefined,
  directValue: string | undefined,
  name: string,
): Promise<string> {
  const storedValue = binding ? await binding.get() : undefined;
  const value = storedValue?.trim() || directValue?.trim();
  if (!value) {
    throw new Error(`${name} is not configured`);
  }
  return value;
}
