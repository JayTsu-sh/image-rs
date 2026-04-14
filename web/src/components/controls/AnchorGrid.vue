<script setup lang="ts">
import type { Anchor } from '../../types';

defineProps<{ modelValue: Anchor }>();
defineEmits<{ (e: 'update:modelValue', v: Anchor): void }>();

const ROWS: Anchor[][] = [
  ['top_left', 'top', 'top_right'],
  ['left', 'center', 'right'],
  ['bottom_left', 'bottom', 'bottom_right'],
];
</script>

<template>
  <div class="space-y-2">
    <label
      class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
    >
      Position
    </label>
    <div class="grid grid-cols-3 gap-1 w-24 mx-auto">
      <template v-for="(row, rowIdx) in ROWS" :key="rowIdx">
        <button
          v-for="a in row"
          :key="a"
          type="button"
          :title="a"
          :class="[
            'w-7 h-7 rounded-sm border transition-colors',
            modelValue === a
              ? 'bg-primary border-primary/30 shadow-inner'
              : 'bg-surface-container border-outline-variant/20 hover:bg-primary/10',
          ]"
          @click="$emit('update:modelValue', a)"
        />
      </template>
    </div>
  </div>
</template>
