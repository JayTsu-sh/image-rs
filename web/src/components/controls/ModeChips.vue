<script setup lang="ts" generic="T extends string">
defineProps<{
  label: string;
  modelValue: T | undefined;
  options: readonly T[];
}>();
defineEmits<{ (e: 'update:modelValue', v: T): void }>();
</script>

<template>
  <div class="space-y-1">
    <label
      class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
    >
      {{ label }}
    </label>
    <div class="flex gap-1 flex-wrap">
      <button
        v-for="opt in options"
        :key="opt"
        type="button"
        :class="[
          'flex-1 min-w-[3rem] py-1.5 text-[11px] font-medium rounded-md transition-colors uppercase tracking-wide',
          modelValue === opt
            ? 'bg-primary text-on-primary'
            : 'bg-surface-container-low text-on-surface-variant hover:bg-surface-container',
        ]"
        @click="$emit('update:modelValue', opt)"
      >
        {{ opt }}
      </button>
    </div>
  </div>
</template>
