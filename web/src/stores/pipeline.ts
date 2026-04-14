import { defineStore } from 'pinia';
import { computed, reactive, ref, watch } from 'vue';
import { useDebounceFn } from '@vueuse/core';
import { postProcess, postDiff, ApiError } from '../api';
import type { DiffMode, OpDto, OutputDto, ProcessMeta } from '../types';

let opIdCounter = 0;
const nextId = (): string => `op_${++opIdCounter}`;

export interface OpEntry {
  id: string;
  expanded: boolean;
  data: OpDto;
}

export const usePipeline = defineStore('pipeline', () => {
  // ── source image ──────────────────────────────────────────────────────────
  const sourceFile = ref<File | null>(null);
  const sourceUrl = ref<string>('');
  const sourceWidth = ref(0);
  const sourceHeight = ref(0);
  const sourceBytes = ref(0);

  // ── watermark asset (referenced by watermark_image op) ────────────────────
  const watermarkFile = ref<File | null>(null);
  const watermarkUrl = ref<string>('');

  // ── ordered op chain ──────────────────────────────────────────────────────
  const ops = ref<OpEntry[]>([]);

  // ── output ────────────────────────────────────────────────────────────────
  const output = reactive<OutputDto>({
    format: undefined,
    quality: 82,
    lossless: false,
    progressive: false,
  });

  // ── result ────────────────────────────────────────────────────────────────
  const resultBlob = ref<Blob | null>(null);
  const resultUrl = ref<string>('');
  const resultMeta = ref<ProcessMeta | null>(null);

  // ── diff (computed on demand when viewMode === 'diff') ────────────────────
  const diffBlob = ref<Blob | null>(null);
  const diffUrl = ref<string>('');
  const diffMode = ref<DiffMode>('highlight');

  // ── preview view state ────────────────────────────────────────────────────
  const viewMode = ref<'split' | 'overlay' | 'diff' | 'after'>('split');
  const zoom = ref(100);
  const splitPercent = ref(50);
  const overlayOpacity = ref(0.5);

  // ── transient state ───────────────────────────────────────────────────────
  const isProcessing = ref(false);
  const isDiffing = ref(false);
  const error = ref<string | null>(null);

  // Source ↔ result must match to do a pixel diff.
  const canDiff = computed(() => {
    const m = resultMeta.value;
    return (
      sourceWidth.value > 0 &&
      m !== null &&
      sourceWidth.value === m.width &&
      sourceHeight.value === m.height
    );
  });

  // ── computed ──────────────────────────────────────────────────────────────
  const opCount = computed(() => ops.value.length);
  const opsAsDto = computed<OpDto[]>(() => ops.value.map((e) => e.data));

  // ── actions ───────────────────────────────────────────────────────────────
  function setSourceFile(file: File | null) {
    if (sourceUrl.value) URL.revokeObjectURL(sourceUrl.value);
    sourceFile.value = file;
    if (file) {
      sourceUrl.value = URL.createObjectURL(file);
      sourceBytes.value = file.size;
      const img = new Image();
      img.onload = () => {
        sourceWidth.value = img.naturalWidth;
        sourceHeight.value = img.naturalHeight;
      };
      img.src = sourceUrl.value;
    } else {
      sourceUrl.value = '';
      sourceWidth.value = 0;
      sourceHeight.value = 0;
      sourceBytes.value = 0;
    }
    // New source invalidates any existing result
    if (resultUrl.value) URL.revokeObjectURL(resultUrl.value);
    resultBlob.value = null;
    resultUrl.value = '';
    resultMeta.value = null;
  }

  function setWatermarkFile(file: File | null) {
    if (watermarkUrl.value) URL.revokeObjectURL(watermarkUrl.value);
    watermarkFile.value = file;
    watermarkUrl.value = file ? URL.createObjectURL(file) : '';
  }

  function addOp(data: OpDto) {
    ops.value.push({ id: nextId(), expanded: true, data });
  }

  function removeOp(id: string) {
    ops.value = ops.value.filter((o) => o.id !== id);
  }

  function toggleOp(id: string) {
    const op = ops.value.find((o) => o.id === id);
    if (op) op.expanded = !op.expanded;
  }

  function moveOp(fromIndex: number, toIndex: number) {
    if (
      fromIndex < 0 ||
      toIndex < 0 ||
      fromIndex >= ops.value.length ||
      toIndex >= ops.value.length
    )
      return;
    const [item] = ops.value.splice(fromIndex, 1);
    ops.value.splice(toIndex, 0, item);
  }

  async function process() {
    if (!sourceFile.value) return;
    isProcessing.value = true;
    error.value = null;
    try {
      const result = await postProcess(
        sourceFile.value,
        opsAsDto.value,
        { ...output },
        watermarkFile.value,
      );
      if (resultUrl.value) URL.revokeObjectURL(resultUrl.value);
      resultBlob.value = result.blob;
      resultUrl.value = URL.createObjectURL(result.blob);
      resultMeta.value = result.meta;
    } catch (e) {
      const apiErr = e as ApiError;
      error.value = apiErr?.message || String(e);
      resultBlob.value = null;
      resultMeta.value = null;
    } finally {
      isProcessing.value = false;
    }
  }

  function downloadResult() {
    if (!resultBlob.value || !resultUrl.value) return;
    const ext = output.format || 'jpeg';
    const a = document.createElement('a');
    a.href = resultUrl.value;
    a.download = `image-rs-output.${ext === 'jpeg' ? 'jpg' : ext}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  async function computeDiff() {
    if (!sourceFile.value || !resultBlob.value) {
      return;
    }
    if (!canDiff.value) {
      error.value =
        'Diff requires source and result to have identical dimensions';
      return;
    }
    isDiffing.value = true;
    try {
      const res = await postDiff(
        sourceFile.value,
        resultBlob.value,
        diffMode.value,
        'png',
      );
      if (diffUrl.value) URL.revokeObjectURL(diffUrl.value);
      diffBlob.value = res.blob;
      diffUrl.value = URL.createObjectURL(res.blob);
    } catch (e) {
      const apiErr = e as ApiError;
      error.value = apiErr?.message || String(e);
    } finally {
      isDiffing.value = false;
    }
  }

  // ── auto-process: re-run the pipeline 400ms after the last edit ───────────
  const processDebounced = useDebounceFn(() => process(), 400);

  watch(
    [
      () => ops.value.map((e) => e.data),
      () => ({ ...output }),
      sourceFile,
    ],
    () => {
      if (sourceFile.value) processDebounced();
    },
    { deep: true },
  );

  // When result changes (or view mode flips to diff), recompute diff.
  watch([viewMode, resultBlob, diffMode], () => {
    if (viewMode.value === 'split' && resultBlob.value && canDiff.value) {
      // Split and overlay modes don't need the diff image.
    }
    if (diffBlob.value) {
      URL.revokeObjectURL(diffUrl.value);
      diffBlob.value = null;
      diffUrl.value = '';
    }
  });

  return {
    // state
    sourceFile,
    sourceUrl,
    sourceWidth,
    sourceHeight,
    sourceBytes,
    watermarkFile,
    watermarkUrl,
    ops,
    output,
    resultBlob,
    resultUrl,
    resultMeta,
    diffBlob,
    diffUrl,
    diffMode,
    viewMode,
    zoom,
    splitPercent,
    overlayOpacity,
    isProcessing,
    isDiffing,
    error,
    // computed
    opCount,
    opsAsDto,
    canDiff,
    // actions
    setSourceFile,
    setWatermarkFile,
    addOp,
    removeOp,
    toggleOp,
    moveOp,
    process,
    computeDiff,
    downloadResult,
  };
});
