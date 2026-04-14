<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'sharpen' }>;
const data = props.entry.data as Data;

if (data.radius === undefined) data.radius = 1.0;
</script>

<template>
  <OperationCard :entry="entry" icon="deblur" title="Sharpen" :badge="`×${data.amount}`">
    <LabeledSlider
      :model-value="data.amount"
      label="Amount"
      :min="0"
      :max="5"
      :step="0.05"
      :format="(v) => v.toFixed(2)"
      @update:model-value="(v) => (data.amount = v)"
    />
    <LabeledSlider
      :model-value="data.radius ?? 1.0"
      label="Radius"
      :min="0.1"
      :max="10"
      :step="0.1"
      :format="(v) => v.toFixed(1)"
      @update:model-value="(v) => (data.radius = v)"
    />
  </OperationCard>
</template>
