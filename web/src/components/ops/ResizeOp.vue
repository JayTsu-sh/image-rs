<script setup lang="ts">
import { watch } from 'vue';
import OperationCard from '../OperationCard.vue';
import NumberField from '../controls/NumberField.vue';
import ModeChips from '../controls/ModeChips.vue';
import { usePipeline } from '../../stores/pipeline';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto, ResizeMode, Interpolation } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'resize' }>;
const data = props.entry.data as Data;

const pipeline = usePipeline();

const MODES: readonly ResizeMode[] = ['fit', 'fill', 'exact'] as const;
const INTERP: readonly Interpolation[] = [
  'auto',
  'nearest',
  'linear',
  'cubic',
  'area',
  'lanczos4',
] as const;

// Cross-field invariant (mirrors backend ResizeSpec::new):
// `fit` accepts a single dimension; `exact` and `fill` require both.
// When the user switches mode to exact/fill, auto-populate the missing
// side from the source image (or a sensible fallback) so the pipeline
// doesn't 422 on the very next auto-process cycle.
watch(
  () => data.mode,
  (m) => {
    if (m === 'exact' || m === 'fill') {
      const sw = pipeline.sourceWidth;
      const sh = pipeline.sourceHeight;
      if (data.width === undefined && data.height === undefined) {
        data.width = sw || 800;
        data.height = sh || 600;
      } else if (data.width === undefined) {
        data.width =
          sw && sh && data.height
            ? Math.round((data.height * sw) / sh)
            : data.height;
      } else if (data.height === undefined) {
        data.height =
          sw && sh && data.width
            ? Math.round((data.width * sh) / sw)
            : data.width;
      }
    }
  },
);
</script>

<template>
  <OperationCard
    :entry="entry"
    icon="aspect_ratio"
    title="Resize"
    :badge="data.interpolation && data.interpolation !== 'auto' ? data.interpolation : undefined"
  >
    <div class="grid grid-cols-2 gap-3">
      <NumberField v-model="data.width" label="Width" :min="1" :max="16384" />
      <NumberField v-model="data.height" label="Height" :min="1" :max="16384" />
    </div>
    <ModeChips v-model="data.mode" label="Mode" :options="MODES" />
    <div class="space-y-1">
      <label
        class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
      >
        Interpolation
      </label>
      <select
        v-model="data.interpolation"
        class="w-full bg-surface-container-low border-0 rounded-md px-3 py-1.5 text-sm focus:ring-1 focus:ring-primary focus:outline-none"
      >
        <option v-for="i in INTERP" :key="i" :value="i">{{ i }}</option>
      </select>
    </div>
  </OperationCard>
</template>
