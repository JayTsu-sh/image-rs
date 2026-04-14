<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import ColorField from '../controls/ColorField.vue';
import AnchorGrid from '../controls/AnchorGrid.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'watermark_text' }>;
const data = props.entry.data as Data;

if (!data.color) data.color = '#ffffffff';
if (data.position === undefined) data.position = 'bottom_right';
if (data.size === undefined) data.size = 24;
if (data.margin === undefined) data.margin = 16;
if (data.shadow === undefined) data.shadow = false;
</script>

<template>
  <OperationCard :entry="entry" icon="title" title="Text watermark">
    <div class="space-y-1">
      <label
        class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
      >
        Text
      </label>
      <input
        v-model="data.text"
        type="text"
        class="w-full bg-surface-container-low border-0 rounded-md px-3 py-1.5 text-sm focus:ring-1 focus:ring-primary focus:outline-none"
        placeholder="© 2026"
      />
    </div>

    <div class="space-y-1">
      <label
        class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
      >
        Font
      </label>
      <input
        v-model="data.font"
        type="text"
        class="w-full bg-surface-container-low border-0 rounded-md px-3 py-1.5 text-sm font-mono focus:ring-1 focus:ring-primary focus:outline-none"
        placeholder="DejaVuSans"
      />
    </div>

    <ColorField
      :model-value="data.color ?? '#ffffffff'"
      label="Color"
      @update:model-value="(v) => (data.color = v)"
    />

    <AnchorGrid
      :model-value="data.position ?? 'bottom_right'"
      @update:model-value="(v) => (data.position = v)"
    />

    <LabeledSlider
      :model-value="data.size ?? 24"
      label="Size"
      :min="4"
      :max="256"
      :step="1"
      :format="(v) => `${v}px`"
      @update:model-value="(v) => (data.size = v)"
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

    <div class="flex items-center justify-between">
      <span class="text-xs font-medium">Shadow</span>
      <button
        type="button"
        class="relative w-10 h-5 rounded-full transition-colors"
        :class="data.shadow ? 'bg-primary/20' : 'bg-surface-container-highest'"
        @click="data.shadow = !data.shadow"
      >
        <div
          class="absolute top-1 w-3 h-3 rounded-full transition-all"
          :class="data.shadow ? 'right-1 bg-primary' : 'left-1 bg-white'"
        />
      </button>
    </div>
  </OperationCard>
</template>
