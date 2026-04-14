<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'saturation' }>;
const data = props.entry.data as Data;
</script>

<template>
  <OperationCard :entry="entry" icon="palette" title="Saturation" :badge="`${data.factor.toFixed(2)}x`">
    <LabeledSlider
      :model-value="data.factor"
      label="Factor"
      :min="0"
      :max="4"
      :step="0.05"
      :format="(v) => `${v.toFixed(2)}x`"
      @update:model-value="(v) => (data.factor = v)"
    />
  </OperationCard>
</template>
