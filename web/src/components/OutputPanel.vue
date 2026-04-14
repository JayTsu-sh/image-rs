<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { usePipeline } from '../stores/pipeline';
import { buildUrlDsl, hasImageWatermark } from '../url-dsl';
import type { ImageFormat } from '../types';

const pipeline = usePipeline();

const FORMATS: { key: ImageFormat; label: string }[] = [
  { key: 'jpeg', label: 'JPG' },
  { key: 'png', label: 'PNG' },
  { key: 'webp', label: 'WEBP' },
];

// JPEG is inherently lossy; WebP's lossless path is separate from progressive.
const losslessDisabled = computed(() => pipeline.output.format === 'jpeg');
const progressiveDisabled = computed(
  () => pipeline.output.format === 'webp',
);
const qualityDisabled = computed(() => pipeline.output.lossless === true);

// Enforce domain invariants the backend also checks — nicer UX to clear
// flags automatically than let the backend bounce the request.
watch(
  () => pipeline.output.format,
  (f) => {
    if (f === 'jpeg' && pipeline.output.lossless) {
      pipeline.output.lossless = false;
    }
    if (f === 'webp' && pipeline.output.progressive) {
      pipeline.output.progressive = false;
    }
  },
);
watch(
  () => pipeline.output.lossless,
  (l) => {
    if (l && pipeline.output.format === 'jpeg') {
      pipeline.output.lossless = false;
    }
  },
);

const urlDsl = computed(() =>
  buildUrlDsl(pipeline.opsAsDto, pipeline.output),
);
const watermarkImgInPipeline = computed(() =>
  hasImageWatermark(pipeline.opsAsDto),
);

const copyState = ref<'idle' | 'copied'>('idle');
async function copyUrlDsl() {
  if (!urlDsl.value) return;
  try {
    await navigator.clipboard.writeText(urlDsl.value);
    copyState.value = 'copied';
    setTimeout(() => (copyState.value = 'idle'), 1200);
  } catch {
    // ignore
  }
}
</script>

<template>
  <aside
    class="w-[300px] bg-surface-container-low flex flex-col border-l border-outline-variant/20 p-6 overflow-y-auto"
  >
    <h2 class="text-sm font-semibold mb-6">Output Settings</h2>

    <!-- Format chips -->
    <div class="space-y-3 mb-8">
      <label
        class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
      >
        File Format
      </label>
      <div class="grid grid-cols-3 gap-2">
        <label
          v-for="fmt in FORMATS"
          :key="fmt.key"
          class="cursor-pointer"
        >
          <input
            v-model="pipeline.output.format"
            class="sr-only peer"
            name="format"
            type="radio"
            :value="fmt.key"
          />
          <div
            class="text-center py-2 rounded-lg border bg-surface-container-lowest border-outline-variant/20 peer-checked:bg-primary peer-checked:text-white peer-checked:border-primary transition-all"
          >
            <span class="text-xs font-semibold">{{ fmt.label }}</span>
          </div>
        </label>
      </div>
    </div>

    <!-- Quality & toggles -->
    <div class="space-y-6 mb-10">
      <div class="space-y-3">
        <div class="flex justify-between items-center">
          <label
            class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
            :class="{ 'opacity-50': qualityDisabled }"
          >
            Quality
          </label>
          <span
            class="font-mono text-xs font-semibold"
            :class="{ 'opacity-50': qualityDisabled }"
          >
            {{ pipeline.output.quality ?? 82 }}
          </span>
        </div>
        <input
          type="range"
          class="w-full"
          min="1"
          max="100"
          :disabled="qualityDisabled"
          :value="pipeline.output.quality ?? 82"
          @input="
            (e) =>
              (pipeline.output.quality = Number(
                (e.target as HTMLInputElement).value,
              ))
          "
        />
      </div>

      <div class="space-y-4">
        <!-- Lossless -->
        <div
          class="flex items-center justify-between"
          :class="{ 'opacity-50': losslessDisabled }"
        >
          <span class="text-sm font-medium">Lossless Compression</span>
          <button
            type="button"
            class="relative w-10 h-5 rounded-full transition-colors"
            :class="[
              pipeline.output.lossless
                ? 'bg-primary/20'
                : 'bg-surface-container-highest',
              { 'cursor-not-allowed': losslessDisabled },
            ]"
            :disabled="losslessDisabled"
            @click="pipeline.output.lossless = !pipeline.output.lossless"
          >
            <div
              class="absolute top-1 w-3 h-3 rounded-full transition-all"
              :class="
                pipeline.output.lossless
                  ? 'right-1 bg-primary'
                  : 'left-1 bg-white'
              "
            />
          </button>
        </div>

        <!-- Progressive -->
        <div
          class="flex items-center justify-between"
          :class="{ 'opacity-50': progressiveDisabled }"
        >
          <span class="text-sm font-medium">Progressive Loading</span>
          <button
            type="button"
            class="relative w-10 h-5 rounded-full transition-colors"
            :class="[
              pipeline.output.progressive
                ? 'bg-primary/20'
                : 'bg-surface-container-highest',
              { 'cursor-not-allowed': progressiveDisabled },
            ]"
            :disabled="progressiveDisabled"
            @click="
              pipeline.output.progressive = !pipeline.output.progressive
            "
          >
            <div
              class="absolute top-1 w-3 h-3 rounded-full transition-all"
              :class="
                pipeline.output.progressive
                  ? 'right-1 bg-primary'
                  : 'left-1 bg-white'
              "
            />
          </button>
        </div>
      </div>
    </div>

    <!-- Error toast -->
    <div
      v-if="pipeline.error"
      class="mb-4 p-3 bg-error-container text-on-error-container rounded-lg text-xs"
    >
      <div class="font-semibold mb-1">Error</div>
      {{ pipeline.error }}
    </div>

    <!-- Actions -->
    <div class="space-y-3 mt-auto">
      <button
        class="w-full bg-gradient-to-br from-primary to-primary-dim text-white py-4 rounded-xl font-bold shadow-lg shadow-primary/20 hover:scale-[0.98] transition-all flex items-center justify-center gap-3 disabled:opacity-50 disabled:hover:scale-100 disabled:cursor-not-allowed"
        :disabled="!pipeline.sourceFile || pipeline.isProcessing"
        @click="pipeline.process"
      >
        <span
          v-if="pipeline.isProcessing"
          class="material-symbols-outlined animate-spin"
        >
          progress_activity
        </span>
        <span v-else class="material-symbols-outlined">play_arrow</span>
        {{ pipeline.isProcessing ? 'Processing…' : 'Process Image' }}
      </button>
      <button
        class="w-full bg-surface-container-high text-on-surface py-4 rounded-xl font-semibold hover:bg-surface-container-highest transition-colors flex items-center justify-center gap-3 disabled:opacity-50 disabled:cursor-not-allowed"
        :disabled="!pipeline.resultBlob"
        @click="pipeline.downloadResult"
      >
        <span class="material-symbols-outlined">download</span>
        Download
      </button>
    </div>

    <!-- Share as URL DSL -->
    <div class="mt-8 border-t border-outline-variant/20 pt-6">
      <div class="flex items-center justify-between mb-4">
        <span class="text-xs font-semibold text-on-surface-variant">
          Share as URL
        </span>
        <button
          class="text-primary text-[10px] font-bold uppercase disabled:opacity-40"
          :disabled="!urlDsl"
          @click="copyUrlDsl"
        >
          {{ copyState === 'copied' ? 'Copied!' : 'Copy' }}
        </button>
      </div>
      <div class="bg-surface-container-highest rounded-lg p-3">
        <code
          class="font-mono text-[10px] text-on-surface-variant break-all leading-relaxed"
        >
          <template v-if="urlDsl">
            GET /v1/img/{key}?p={{ urlDsl }}
          </template>
          <template v-else>
            (pipeline is empty)
          </template>
        </code>
      </div>
      <p
        v-if="watermarkImgInPipeline"
        class="mt-2 text-[10px] text-on-surface-variant/70 italic"
      >
        Note: image watermark cannot be expressed in the URL DSL — use
        POST /v1/process for binary assets.
      </p>
    </div>
  </aside>
</template>
