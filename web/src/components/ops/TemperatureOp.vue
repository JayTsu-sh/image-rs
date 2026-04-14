<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'temperature' }>;
const data = props.entry.data as Data;
</script>

<template>
  <OperationCard
    :entry="entry"
    icon="thermostat"
    title="Temperature"
    :badge="data.value > 0 ? `+${data.value}` : String(data.value)"
  >
    <LabeledSlider
      :model-value="data.value"
      label="Warm / Cool"
      :min="-100"
      :max="100"
      :step="1"
      :format="(v) => (v > 0 ? `+${v}` : String(v))"
      @update:model-value="(v) => (data.value = v)"
    />
  </OperationCard>
</template>
