<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'round_corner' }>;
const data = props.entry.data as Data;
</script>

<template>
  <OperationCard :entry="entry" icon="rounded_corner" title="Round corner" :badge="`${data.radius}px`">
    <LabeledSlider
      :model-value="data.radius"
      label="Radius"
      :min="0"
      :max="500"
      :step="1"
      :format="(v) => `${v}px`"
      @update:model-value="(v) => (data.radius = v)"
    />
    <p class="text-[10px] text-on-surface-variant">
      Requires PNG or WebP output (alpha channel).
    </p>
  </OperationCard>
</template>
