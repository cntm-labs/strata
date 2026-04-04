import { describe, it, expect, vi } from 'vitest'
import { shallowMount } from '@vue/test-utils'

// Stub heavy deps before importing PanelRenderer (which transitively imports uPlot etc.)
vi.mock('uplot', () => ({
  default: vi.fn().mockImplementation(() => ({ destroy: vi.fn() })),
}))
vi.mock('uplot/dist/uPlot.min.css', () => ({}))
vi.mock('vue-echarts', () => ({
  default: { name: 'VChart', template: '<div />', props: ['option'] },
}))
vi.mock('echarts/core', () => ({ use: vi.fn() }))
vi.mock('echarts/charts', () => ({ GaugeChart: {}, BarChart: {}, HeatmapChart: {}, PieChart: {} }))
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

import PanelRenderer from '../PanelRenderer.vue'
import type { Panel } from '@/types'

function makePanel(type: string): Panel {
  return {
    id: '1',
    dashboard_id: 'd1',
    title: `${type} Panel`,
    type: type as Panel['type'],
    query: 'up',
    config: {},
    position: { x: 0, y: 0, w: 6, h: 3, i: '1' },
    created_at: '',
    updated_at: '',
  }
}

describe('PanelRenderer', () => {
  it('renders panel title', () => {
    const wrapper = shallowMount(PanelRenderer, {
      props: { panel: makePanel('stat'), data: null },
    })
    expect(wrapper.find('h3').text()).toBe('stat Panel')
  })

  it('shows edit button when editable', () => {
    const wrapper = shallowMount(PanelRenderer, {
      props: { panel: makePanel('stat'), data: null, editable: true },
    })
    expect(wrapper.find('button').exists()).toBe(true)
  })

  it('hides edit button when not editable', () => {
    const wrapper = shallowMount(PanelRenderer, {
      props: { panel: makePanel('stat'), data: null, editable: false },
    })
    expect(wrapper.find('button').exists()).toBe(false)
  })

  it('emits edit event when button clicked', async () => {
    const panel = makePanel('stat')
    const wrapper = shallowMount(PanelRenderer, {
      props: { panel, data: null, editable: true },
    })
    await wrapper.find('button').trigger('click')
    expect(wrapper.emitted('edit')?.[0]).toEqual([panel])
  })

  const panelTypes = ['timeseries', 'stat', 'gauge', 'table', 'bar', 'heatmap', 'logs', 'piechart']
  for (const type of panelTypes) {
    it(`renders ${type} panel without error`, () => {
      const wrapper = shallowMount(PanelRenderer, {
        props: { panel: makePanel(type), data: null },
      })
      expect(wrapper.exists()).toBe(true)
    })
  }

  it('falls back to StatPanel for unknown type', () => {
    const wrapper = shallowMount(PanelRenderer, {
      props: { panel: makePanel('unknown'), data: null },
    })
    expect(wrapper.exists()).toBe(true)
  })
})
