<script setup lang="ts">
import { ref } from 'vue';
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import AnchorGrid from '../controls/AnchorGrid.vue';
import { usePipeline } from '../../stores/pipeline';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'watermark_image' }>;
const data = props.entry.data as Data;

const pipeline = usePipeline();
const fileInput = ref<HTMLInputElement | null>(null);

function onFileChange(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (file) {
    pipeline.setWatermarkFile(file);
    // Ensure the op references the canonical asset name the extractor uses.
    data.asset = 'watermark';
  }
}
</script>

<template>
  <OperationCard :entry="entry" icon="branding_watermark" title="Image watermark">
    <!-- Watermark file picker -->
    <div class="space-y-1">
      <label
        class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
      >
        Watermark Image
      </label>
      <div
        class="flex items-center gap-2 bg-surface-container-low rounded-md p-2 cursor-pointer hover:bg-surface-container transition-colors"
        @click="fileInput?.click()"
      >
        <div
          class="w-10 h-10 bg-surface-container-highest rounded overflow-hidden shrink-0 flex items-center justify-center"
        >
          <img
            v-if="pipeline.watermarkUrl"
            :src="pipeline.watermarkUrl"
            class="w-full h-full object-contain"
          />
          <span
            v-else
            class="material-symbols-outlined text-on-surface-variant text-[18px]"
          >
            image
          </span>
        </div>
        <span class="text-xs text-on-surface-variant truncate">
          {{ pipeline.watermarkFile?.name || 'Click to choose file' }}
        </span>
      </div>
      <input
        ref="fileInput"
        type="file"
        accept="image/png,image/webp,image/jpeg"
        class="hidden"
        @change="onFileChange"
      />
    </div>

    <AnchorGrid
      :model-value="data.position ?? 'bottom_right'"
      @update:model-value="(v) => (data.position = v)"
    />

    <LabeledSlider
      :model-value="data.opacity ?? 1.0"
      label="Opacity"
      :min="0"
      :max="1"
      :step="0.01"
      :format="(v) => `${Math.round(v * 100)}%`"
      @update:model-value="(v) => (data.opacity = v)"
    />
    <LabeledSlider
      :model-value="data.scale ?? 0.2"
      label="Scale"
      :min="0.05"
      :max="4"
      :step="0.01"
      :format="(v) => `${Math.round(v * 100)}%`"
      @update:model-value="(v) => (data.scale = v)"
    />
    <LabeledSlider
      :model-value="data.margin ?? 16"
      label="Margin"
      :min="0"
      :max="100"
      :step="1"
      :format="(v) => `${v}px`"
      @update:model-value="(v) => (data.margin = v)"
    />
  </OperationCard>
</template>
