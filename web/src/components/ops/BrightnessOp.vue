<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'brightness' }>;
const data = props.entry.data as Data;
</script>

<template>
  <OperationCard :entry="entry" icon="wb_sunny" title="Brightness" :badge="data.value > 0 ? `+${data.value}` : `${data.value}`">
    <LabeledSlider
      :model-value="data.value"
      label="Value"
      :min="-255"
      :max="255"
      :step="1"
      :format="(v) => (v > 0 ? `+${v}` : String(v))"
      @update:model-value="(v) => (data.value = v)"
    />
  </OperationCard>
</template>
