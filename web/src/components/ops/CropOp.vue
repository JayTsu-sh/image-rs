<script setup lang="ts">
import OperationCard from '../OperationCard.vue';
import NumberField from '../controls/NumberField.vue';
import type { OpEntry } from '../../stores/pipeline';
import type { OpDto } from '../../types';

const props = defineProps<{ entry: OpEntry }>();
type Data = Extract<OpDto, { op: 'crop' }>;
const data = props.entry.data as Data;
</script>

<template>
  <OperationCard :entry="entry" icon="crop" title="Crop">
    <div class="grid grid-cols-2 gap-3">
      <NumberField
        :model-value="data.x"
        label="X"
        :min="0"
        @update:model-value="(v) => (data.x = v ?? 0)"
      />
      <NumberField
        :model-value="data.y"
        label="Y"
        :min="0"
        @update:model-value="(v) => (data.y = v ?? 0)"
      />
      <NumberField
        :model-value="data.width"
        label="Width"
        :min="1"
        @update:model-value="(v) => (data.width = v ?? 1)"
      />
      <NumberField
        :model-value="data.height"
        label="Height"
        :min="1"
        @update:model-value="(v) => (data.height = v ?? 1)"
      />
    </div>
  </OperationCard>
</template>
