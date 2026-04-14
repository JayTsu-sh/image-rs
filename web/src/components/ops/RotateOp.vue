<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import LabeledSlider from '../controls/LabeledSlider.vue';
import ColorField from '../controls/ColorField.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'rotate' }>;
const data = props.entry.data as Data;

// Seed background so ColorField has something to bind to.
if (!data.background) data.background = '#00000000';
</script>

<template>
  <OperationCard :entry="entry" icon="rotate_right" title="Rotate" :badge="`${data.angle}°`">
    <LabeledSlider
      :model-value="data.angle"
      label="Angle"
      :min="-180"
      :max="180"
      :step="1"
      :format="(v) => `${v}°`"
      @update:model-value="(v) => (data.angle = v)"
    />
    <ColorField
      :model-value="data.background ?? '#00000000'"
      label="Background"
      @update:model-value="(v) => (data.background = v)"
    />
  </OperationCard>
</template>
