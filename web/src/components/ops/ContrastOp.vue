<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'contrast' }>;
const data = props.entry.data as Data;
</script>

<template>
  <OperationCard :entry="entry" icon="contrast" title="Contrast" :badge="`${data.value.toFixed(2)}x`">
    <LabeledSlider
      :model-value="data.value"
      label="Value"
      :min="0"
      :max="4"
      :step="0.05"
      :format="(v) => `${v.toFixed(2)}x`"
      @update:model-value="(v) => (data.value = v)"
    />
  </OperationCard>
</template>
