// Thin fetch wrapper. The vite dev server proxies /v1/* to the rust
// backend on :8080; in production both are served from the same origin.

import type {
  DiffMode,
  ImageFormat,
  OpDto,
  OutputDto,
  ProcessMeta,
  ProcessResult,
} from './types';

const API_BASE = '';

export class ApiError extends Error {
  status: number;
  code?: string;
  op?: string;
  constructor(status: number, msg: string, code?: string, op?: string) {
    super(msg);
    this.status = status;
    this.code = code;
    this.op = op;
  }
}

async function readError(res: Response): Promise<ApiError> {
  try {
    const json = await res.json();
    return new ApiError(
      res.status,
      json.message || res.statusText,
      json.error,
      json.op,
    );
  } catch {
    return new ApiError(res.status, res.statusText);
  }
}

function metaFromHeaders(res: Response): ProcessMeta {
  const get = (k: string) => res.headers.get(k);
  return {
    width: parseInt(get('x-image-width') || '0', 10),
    height: parseInt(get('x-image-height') || '0', 10),
    bytes: parseInt(get('x-image-bytes') || '0', 10),
    elapsedMs: parseInt(get('x-process-time-ms') || '0', 10),
    cache: (get('x-image-cache') as 'hit' | 'miss' | null) || undefined,
  };
}

export async function postProcess(
  file: File,
  ops: OpDto[],
  output: OutputDto,
  watermark?: File | null,
): Promise<ProcessResult> {
  const fd = new FormData();
  fd.append('file', file);
  if (watermark) fd.append('watermark', watermark);
  fd.append(
    'ops',
    new Blob([JSON.stringify(ops)], { type: 'application/json' }),
  );
  fd.append(
    'output',
    new Blob([JSON.stringify(output)], { type: 'application/json' }),
  );

  const res = await fetch(`${API_BASE}/v1/process`, {
    method: 'POST',
    body: fd,
  });
  if (!res.ok) throw await readError(res);

  const blob = await res.blob();
  return { blob, meta: metaFromHeaders(res) };
}

export async function postDiff(
  before: File | Blob,
  after: File | Blob,
  mode: DiffMode = 'highlight',
  format: ImageFormat = 'webp',
  threshold = 10,
): Promise<ProcessResult> {
  const fd = new FormData();
  fd.append('before', before);
  fd.append('after', after);

  const params = new URLSearchParams({
    mode,
    format,
    threshold: String(threshold),
  });
  const res = await fetch(`${API_BASE}/v1/diff?${params}`, {
    method: 'POST',
    body: fd,
  });
  if (!res.ok) throw await readError(res);

  const blob = await res.blob();
  return { blob, meta: metaFromHeaders(res) };
}
