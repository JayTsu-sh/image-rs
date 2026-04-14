<script setup lang="ts">
defineProps<{
  label: string;
  modelValue: number | undefined;
  min?: number;
  max?: number;
  step?: number;
}>();
defineEmits<{ (e: 'update:modelValue', v: number | undefined): void }>();
</script>

<template>
  <div class="space-y-1">
    <label
      class="text-[10px] font-mono uppercase text-on-surface-variant tracking-wider"
    >
      {{ label }}
    </label>
    <input
      type="number"
      :value="modelValue ?? ''"
      :min="min"
      :max="max"
      :step="step ?? 1"
      class="w-full bg-surface-container-low border-0 rounded-md px-3 py-1.5 text-sm font-mono focus:ring-1 focus:ring-primary focus:outline-none"
      @input="
        (e) => {
          const v = (e.target as HTMLInputElement).value;
          $emit('update:modelValue', v === '' ? undefined : Number(v));
        }
      "
    />
  </div>
</template>
