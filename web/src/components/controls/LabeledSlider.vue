<script setup lang="ts">
interface Props {
  label: string;
  modelValue: number;
  min?: number;
  max?: number;
  step?: number;
  format?: (v: number) => string;
}
const props = withDefaults(defineProps<Props>(), {
  min: 0,
  max: 100,
  step: 1,
  format: (v: number) => String(v),
});
defineEmits<{ (e: 'update:modelValue', v: number): void }>();
void props;
</script>

<template>
  <div class="space-y-2">
    <div class="flex justify-between items-center">
      <span class="text-xs font-medium">{{ label }}</span>
      <span class="font-mono text-xs text-primary">{{ format(modelValue) }}</span>
    </div>
    <input
      type="range"
      class="w-full"
      :min="min"
      :max="max"
      :step="step"
      :value="modelValue"
      @input="
        (e) =>
          $emit(
            'update:modelValue',
            Number((e.target as HTMLInputElement).value),
          )
      "
    />
  </div>
</template>
