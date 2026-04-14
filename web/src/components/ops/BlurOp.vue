<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'blur' }>;
const data = props.entry.data as Data;
</script>

<template>
  <OperationCard :entry="entry" icon="blur_on" title="Blur" :badge="`σ ${data.sigma}`">
    <LabeledSlider
      :model-value="data.sigma"
      label="Sigma"
      :min="0"
      :max="50"
      :step="0.1"
      :format="(v) => v.toFixed(1)"
      @update:model-value="(v) => (data.sigma = v)"
    />
  </OperationCard>
</template>
