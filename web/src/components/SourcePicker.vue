<script setup lang="ts">
import { ref } from 'vue';
import { usePipeline } from '../stores/pipeline';

const pipeline = usePipeline();
const fileInput = ref<HTMLInputElement | null>(null);
const isDragging = ref(false);

function onFileChange(e: Event) {
  const target = e.target as HTMLInputElement;
  if (target.files?.[0]) {
    pipeline.setSourceFile(target.files[0]);
  }
  // Allow re-selecting the same file later
  if (target) target.value = '';
}

function onDrop(e: DragEvent) {
  e.preventDefault();
  isDragging.value = false;
  const file = e.dataTransfer?.files?.[0];
  if (file && file.type.startsWith('image/')) {
    pipeline.setSourceFile(file);
  }
}

function openFileDialog() {
  fileInput.value?.click();
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
</script>

<template>
  <div class="p-4 space-y-4">
    <!-- Loaded state -->
    <div
      v-if="pipeline.sourceFile"
      class="bg-surface-container-lowest rounded-xl p-3 cursor-pointer hover:bg-surface-container-low transition-colors"
      @click="openFileDialog"
    >
      <div class="flex items-center gap-3">
        <div
          class="w-16 h-16 rounded bg-surface-container-highest overflow-hidden shrink-0"
        >
          <img
            :src="pipeline.sourceUrl"
            class="w-full h-full object-cover"
            alt="source thumbnail"
          />
        </div>
        <div class="overflow-hidden flex-1">
          <h3 class="text-sm font-semibold truncate">
            {{ pipeline.sourceFile.name }}
          </h3>
          <p class="font-mono text-[10px] text-on-surface-variant uppercase mt-1">
            {{ formatBytes(pipeline.sourceBytes) }} •
            {{ pipeline.sourceWidth }}×{{ pipeline.sourceHeight }}
          </p>
        </div>
      </div>
    </div>

    <!-- Empty / drop-zone state -->
    <div
      v-else
      class="bg-surface-container-lowest rounded-xl p-6 border-2 border-dashed cursor-pointer transition-colors"
      :class="
        isDragging
          ? 'border-primary bg-primary/5'
          : 'border-outline-variant/30 hover:border-outline-variant/60'
      "
      @click="openFileDialog"
      @dragenter.prevent="isDragging = true"
      @dragover.prevent="isDragging = true"
      @dragleave.prevent="isDragging = false"
      @drop="onDrop"
    >
      <div class="text-center">
        <span
          class="material-symbols-outlined text-3xl text-on-surface-variant"
        >
          add_photo_alternate
        </span>
        <p class="text-xs text-on-surface-variant mt-2">
          Drop image or click to browse
        </p>
        <p
          class="text-[10px] font-mono text-on-surface-variant/70 mt-1 uppercase tracking-wider"
        >
          JPEG · PNG · WebP
        </p>
      </div>
    </div>

    <input
      ref="fileInput"
      type="file"
      accept="image/jpeg,image/png,image/webp"
      class="hidden"
      @change="onFileChange"
    />

    <!-- Active pipeline header strip -->
    <div class="flex items-center justify-between px-1">
      <span
        class="text-[10px] font-mono font-medium text-on-surface-variant uppercase tracking-widest"
      >
        Active Pipeline
      </span>
      <span class="text-[10px] font-mono font-medium text-primary uppercase">
        {{ pipeline.opCount }} {{ pipeline.opCount === 1 ? 'Step' : 'Steps' }}
      </span>
    </div>
  </div>
</template>
