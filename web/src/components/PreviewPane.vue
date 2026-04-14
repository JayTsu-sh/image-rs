<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { usePipeline } from '../stores/pipeline';

const pipeline = usePipeline();
const containerRef = ref<HTMLElement | null>(null);
const isDraggingSplit = ref(false);

const MODES = [
  { key: 'split', label: 'Split' },
  { key: 'overlay', label: 'Overlay' },
  { key: 'diff', label: 'Diff' },
  { key: 'after', label: 'Result' },
] as const;

// When entering diff mode, kick off a diff request if we have fresh data.
watch(
  () => pipeline.viewMode,
  async (mode) => {
    if (mode === 'diff' && pipeline.resultBlob && !pipeline.diffBlob) {
      await pipeline.computeDiff();
    }
  },
);
// Also recompute when the result changes while already in diff mode.
watch(
  () => pipeline.resultBlob,
  async () => {
    if (pipeline.viewMode === 'diff' && pipeline.resultBlob) {
      await pipeline.computeDiff();
    }
  },
);

function startSplitDrag(e: MouseEvent) {
  e.preventDefault();
  isDraggingSplit.value = true;
}
function onMouseMove(e: MouseEvent) {
  if (!isDraggingSplit.value || !containerRef.value) return;
  const rect = containerRef.value.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const pct = Math.min(100, Math.max(0, (x / rect.width) * 100));
  pipeline.splitPercent = pct;
}
function endSplitDrag() {
  isDraggingSplit.value = false;
}

const zoomStyle = computed(() => ({
  transform: `scale(${pipeline.zoom / 100})`,
  transformOrigin: 'center',
}));

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

function fitToWindow() {
  pipeline.zoom = 100;
}
</script>

<template>
  <section
    class="flex-1 flex flex-col bg-surface relative"
    @mousemove="onMouseMove"
    @mouseup="endSplitDrag"
    @mouseleave="endSplitDrag"
  >
    <!-- Toolbar -->
    <div
      class="h-12 flex items-center justify-between px-4 bg-surface-container/30 border-b border-outline-variant/10"
    >
      <!-- View mode toggle -->
      <div class="flex bg-surface-container-high rounded-lg p-0.5">
        <button
          v-for="mode in MODES"
          :key="mode.key"
          :disabled="mode.key === 'diff' && !pipeline.canDiff"
          :class="[
            'px-3 py-1 text-xs font-medium rounded-md transition-colors',
            pipeline.viewMode === mode.key
              ? 'bg-surface-container-lowest text-on-surface shadow-sm font-semibold'
              : 'text-on-surface-variant hover:text-on-surface disabled:opacity-40 disabled:cursor-not-allowed',
          ]"
          @click="pipeline.viewMode = mode.key as typeof pipeline.viewMode"
        >
          {{ mode.label }}
        </button>
      </div>

      <!-- Zoom + fit -->
      <div class="flex items-center gap-6">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-sm">zoom_out</span>
          <input
            type="range"
            class="w-24"
            min="10"
            max="200"
            :value="pipeline.zoom"
            @input="
              (e) =>
                (pipeline.zoom = Number(
                  (e.target as HTMLInputElement).value,
                ))
            "
          />
          <span class="material-symbols-outlined text-sm">zoom_in</span>
          <span class="font-mono text-xs w-10 text-right">{{ pipeline.zoom }}%</span>
        </div>
        <button
          class="flex items-center gap-1.5 px-2 py-1 hover:bg-surface-container rounded transition-colors"
          @click="fitToWindow"
        >
          <span class="material-symbols-outlined text-sm">fit_screen</span>
          <span class="text-xs font-medium">Fit</span>
        </button>
      </div>
    </div>

    <!-- Canvas -->
    <div
      class="flex-1 overflow-hidden relative flex items-center justify-center p-8 bg-[#202528]"
    >
      <!-- Empty state -->
      <div
        v-if="!pipeline.sourceUrl"
        class="text-center text-on-surface-variant/60"
      >
        <span class="material-symbols-outlined text-6xl mb-4 block">
          photo_library
        </span>
        <p class="text-sm">Drop an image in the sidebar to start</p>
      </div>

      <!-- Split view -->
      <div
        v-else-if="pipeline.viewMode === 'split'"
        ref="containerRef"
        class="relative w-full h-full select-none"
        :style="zoomStyle"
      >
        <!-- Before layer (source) -->
        <img
          :src="pipeline.sourceUrl"
          class="absolute inset-0 w-full h-full object-contain"
          alt="original"
        />
        <div
          class="absolute top-4 left-4 bg-black/40 backdrop-blur-md px-2 py-1 rounded text-[10px] font-mono text-white/80 uppercase z-10"
        >
          Original
        </div>
        <!-- After layer (result) clipped by splitPercent -->
        <div
          v-if="pipeline.resultUrl"
          class="absolute inset-0"
          :style="{
            clipPath: `inset(0 0 0 ${pipeline.splitPercent}%)`,
          }"
        >
          <img
            :src="pipeline.resultUrl"
            class="absolute inset-0 w-full h-full object-contain"
            alt="processed"
          />
          <div
            class="absolute top-4 right-4 bg-primary/80 backdrop-blur-md px-2 py-1 rounded text-[10px] font-mono text-white uppercase z-10"
          >
            Processed
          </div>
        </div>
        <!-- Split handle -->
        <div
          v-if="pipeline.resultUrl"
          class="absolute top-0 bottom-0 w-0.5 bg-primary cursor-ew-resize z-20"
          :style="{ left: `${pipeline.splitPercent}%` }"
          @mousedown="startSplitDrag"
        >
          <div
            class="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-8 h-8 bg-primary rounded-full flex items-center justify-center shadow-lg"
          >
            <span class="material-symbols-outlined text-white text-sm">
              unfold_more
            </span>
          </div>
        </div>
      </div>

      <!-- Overlay view -->
      <div
        v-else-if="pipeline.viewMode === 'overlay'"
        class="relative w-full h-full"
        :style="zoomStyle"
      >
        <img
          :src="pipeline.sourceUrl"
          class="absolute inset-0 w-full h-full object-contain"
          alt="original"
        />
        <img
          v-if="pipeline.resultUrl"
          :src="pipeline.resultUrl"
          class="absolute inset-0 w-full h-full object-contain"
          :style="{ opacity: pipeline.overlayOpacity }"
          alt="processed"
        />
        <div class="absolute top-4 left-4 right-4 flex justify-center">
          <div
            class="flex items-center gap-3 bg-black/40 backdrop-blur-md px-3 py-1.5 rounded"
          >
            <span class="text-[10px] font-mono text-white/80 uppercase">
              Overlay
            </span>
            <input
              type="range"
              class="w-32"
              min="0"
              max="1"
              step="0.01"
              :value="pipeline.overlayOpacity"
              @input="
                (e) =>
                  (pipeline.overlayOpacity = Number(
                    (e.target as HTMLInputElement).value,
                  ))
              "
            />
            <span class="font-mono text-[10px] text-white/80">
              {{ Math.round(pipeline.overlayOpacity * 100) }}%
            </span>
          </div>
        </div>
      </div>

      <!-- Diff view -->
      <div
        v-else-if="pipeline.viewMode === 'diff'"
        class="relative w-full h-full"
        :style="zoomStyle"
      >
        <img
          v-if="pipeline.diffUrl"
          :src="pipeline.diffUrl"
          class="absolute inset-0 w-full h-full object-contain"
          alt="diff"
        />
        <div
          v-else-if="pipeline.isDiffing"
          class="absolute inset-0 flex items-center justify-center text-white/70"
        >
          <span class="material-symbols-outlined animate-spin mr-2">
            progress_activity
          </span>
          Computing diff…
        </div>
        <div
          v-else
          class="absolute inset-0 flex items-center justify-center text-white/70 text-sm"
        >
          Apply a processing pipeline to see the diff.
        </div>
        <div class="absolute top-4 left-4 flex items-center gap-2">
          <div class="flex bg-black/40 backdrop-blur-md rounded p-0.5">
            <button
              v-for="m in ['highlight', 'abs', 'grayscale'] as const"
              :key="m"
              class="px-2 py-0.5 text-[10px] font-mono uppercase rounded transition-colors"
              :class="
                pipeline.diffMode === m
                  ? 'bg-primary text-white'
                  : 'text-white/60 hover:text-white/90'
              "
              @click="
                () => {
                  pipeline.diffMode = m;
                  pipeline.computeDiff();
                }
              "
            >
              {{ m }}
            </button>
          </div>
        </div>
      </div>

      <!-- After (result only) view -->
      <div
        v-else-if="pipeline.viewMode === 'after'"
        class="relative w-full h-full"
        :style="zoomStyle"
      >
        <img
          :src="pipeline.resultUrl || pipeline.sourceUrl"
          class="absolute inset-0 w-full h-full object-contain"
          alt="result"
        />
        <div
          v-if="pipeline.resultUrl"
          class="absolute top-4 right-4 bg-primary/80 backdrop-blur-md px-2 py-1 rounded text-[10px] font-mono text-white uppercase"
        >
          Processed
        </div>
      </div>
    </div>

    <!-- Status bar -->
    <footer
      class="h-10 bg-surface-container-low border-t border-outline-variant/10 flex items-center justify-between px-6"
    >
      <div class="flex items-center gap-6">
        <div v-if="pipeline.sourceWidth > 0" class="flex items-center gap-2">
          <span
            class="material-symbols-outlined text-[16px] text-on-surface-variant"
          >
            photo_size_select_large
          </span>
          <span class="font-mono text-[10px] text-on-surface-variant uppercase">
            {{ pipeline.sourceWidth }} × {{ pipeline.sourceHeight }} PX
          </span>
        </div>
        <div v-if="pipeline.sourceBytes > 0" class="flex items-center gap-2">
          <span
            class="material-symbols-outlined text-[16px] text-on-surface-variant"
          >
            hard_drive
          </span>
          <span class="font-mono text-[10px] text-on-surface-variant uppercase">
            {{ formatBytes(pipeline.sourceBytes) }}
          </span>
        </div>
      </div>
      <div v-if="pipeline.resultMeta" class="flex items-center gap-6">
        <div class="flex items-center gap-2">
          <span class="material-symbols-outlined text-[16px] text-primary">
            bolt
          </span>
          <span class="font-mono text-[10px] text-on-surface-variant uppercase">
            PROC: {{ pipeline.resultMeta.elapsedMs }}MS
          </span>
        </div>
        <div class="flex items-center gap-2">
          <span class="font-mono text-[10px] text-on-surface-variant uppercase">
            → {{ pipeline.resultMeta.width }} × {{ pipeline.resultMeta.height }}
          </span>
        </div>
        <div class="flex items-center gap-2">
          <span class="font-mono text-[10px] text-on-surface-variant uppercase">
            {{ formatBytes(pipeline.resultMeta.bytes) }}
          </span>
        </div>
      </div>
    </footer>
  </section>
</template>
