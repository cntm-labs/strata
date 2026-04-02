<template>
  <div class="flex items-center justify-center h-full">
    <div class="text-center">
      <div class="text-4xl font-bold">{{ displayValue }}</div>
      <div v-if="config.unit" class="text-sm text-base-content/60">
        {{ config.unit }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>();

const displayValue = computed(() => {
  const result = (props.data as any)?.data?.result;
  if (!result || result.length === 0) return "—";
  const val = result[0]?.value?.[1] || result[0]?.values?.slice(-1)?.[0]?.[1];
  if (val === undefined) return "—";
  const num = parseFloat(val);
  return isNaN(num)
    ? val
    : num.toLocaleString(undefined, { maximumFractionDigits: 2 });
});
</script>
