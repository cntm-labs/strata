import { describe, it, expect, vi } from 'vitest'
import { shallowMount } from '@vue/test-utils'

// Stub heavy third-party deps
vi.mock('uplot', () => ({
  default: vi.fn().mockImplementation(() => ({ destroy: vi.fn() })),
}))
vi.mock('uplot/dist/uPlot.min.css', () => ({}))
vi.mock('vue-echarts', () => ({
  default: { name: 'VChart', template: '<div />', props: ['option'] },
}))
vi.mock('echarts/core', () => ({ use: vi.fn() }))
vi.mock('echarts/charts', () => ({
  GaugeChart: {},
  BarChart: {},
  HeatmapChart: {},
  PieChart: {},
}))
vi.mock('echarts/components', () => ({
  GridComponent: {},
  TooltipComponent: {},
  VisualMapComponent: {},
  LegendComponent: {},
}))
vi.mock('echarts/renderers', () => ({ CanvasRenderer: {} }))
vi.mock('ag-grid-vue3', () => ({
  AgGridVue: { name: 'AgGridVue', template: '<div />', props: ['rowData', 'columnDefs'] },
}))
vi.mock('@xterm/xterm', () => {
  class Terminal {
    open = vi.fn()
    clear = vi.fn()
    writeln = vi.fn()
    dispose = vi.fn()
  }
  return { Terminal }
})
vi.mock('@xterm/xterm/css/xterm.css', () => ({}))

import StatPanel from '../StatPanel.vue'
import GaugePanel from '../GaugePanel.vue'
import BarPanel from '../BarPanel.vue'
import TablePanel from '../TablePanel.vue'
import HeatmapPanel from '../HeatmapPanel.vue'
import PiechartPanel from '../PiechartPanel.vue'
import TimeseriesPanel from '../TimeseriesPanel.vue'
import LogsPanel from '../LogsPanel.vue'

const defaultProps = { data: null, config: {} }

describe('StatPanel', () => {
  it('renders dash when no data', () => {
    const wrapper = shallowMount(StatPanel, { props: defaultProps })
    expect(wrapper.text()).toContain('—')
  })

  it('renders value from prometheus result', () => {
    const wrapper = shallowMount(StatPanel, {
      props: {
        data: { data: { result: [{ value: [1700000000, '42.5'] }] } },
        config: {},
      },
    })
    expect(wrapper.text()).toContain('42.5')
  })

  it('shows unit when configured', () => {
    const wrapper = shallowMount(StatPanel, {
      props: {
        data: { data: { result: [{ value: [1700000000, '99'] }] } },
        config: { unit: '%' },
      },
    })
    expect(wrapper.text()).toContain('%')
  })
})

describe('GaugePanel', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(GaugePanel, { props: defaultProps })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('BarPanel', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(BarPanel, { props: defaultProps })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('TablePanel', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(TablePanel, { props: defaultProps })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('HeatmapPanel', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(HeatmapPanel, { props: defaultProps })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('PiechartPanel', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(PiechartPanel, { props: defaultProps })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('TimeseriesPanel', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(TimeseriesPanel, { props: defaultProps })
    expect(wrapper.exists()).toBe(true)
  })
})

describe('LogsPanel', () => {
  it('mounts without error', () => {
    const wrapper = shallowMount(LogsPanel, { props: defaultProps })
    expect(wrapper.exists()).toBe(true)
  })
})
