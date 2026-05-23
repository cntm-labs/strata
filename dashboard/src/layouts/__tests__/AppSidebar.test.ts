import { describe, it, expect, vi, beforeEach } from 'vitest'
import { shallowMount } from '@vue/test-utils'

const { NucleusMock, fireListeners } = vi.hoisted(() => {
  const listeners: Array<() => void> = []
  const NucleusMock = {
    user: null as {
      id: string
      email: string
      first_name?: string
      last_name?: string
      avatar_url?: string
    } | null,
    organization: null as { id: string; name: string; slug: string } | null,
    session: null as { token: string } | null,
    get isSignedIn() {
      return NucleusMock.user !== null && NucleusMock.session !== null
    },
    getToken: vi.fn(() => NucleusMock.session?.token ?? null),
    signOut: vi.fn(async () => {
      NucleusMock.user = null
      NucleusMock.organization = null
      NucleusMock.session = null
      listeners.forEach((l) => l())
    }),
    addListener: vi.fn((fn: () => void) => {
      listeners.push(fn)
      return () => {
        const i = listeners.indexOf(fn)
        if (i >= 0) listeners.splice(i, 1)
      }
    }),
  }
  function fireListeners() {
    listeners.forEach((l) => l())
  }
  return { NucleusMock, fireListeners }
})

vi.mock('@cntm-labs/nucleus-js', () => ({ Nucleus: NucleusMock }))

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

import AppSidebar from '../AppSidebar.vue'

beforeEach(() => {
  NucleusMock.user = null
  NucleusMock.organization = null
  NucleusMock.session = null
  fireListeners()
})

const mountOpts = {
  global: { stubs: { RouterLink: true } },
}

function signIn(fields: { first_name?: string; last_name?: string; email?: string } = {}) {
  NucleusMock.user = {
    id: '1',
    email: fields.email ?? 'user@test.com',
    first_name: fields.first_name,
    last_name: fields.last_name,
  }
  NucleusMock.session = { token: 'jwt-1' }
  fireListeners()
}

describe('AppSidebar', () => {
  it('renders Strata brand', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.text()).toContain('Strata')
  })

  it('renders all 6 nav links', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    const links = wrapper.findAllComponents({ name: 'RouterLink' })
    expect(links.length).toBe(6)
  })

  it('has correct nav paths', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    const links = wrapper.findAllComponents({ name: 'RouterLink' })
    const paths = links.map((l) => l.attributes('to'))
    expect(paths).toContain('/dashboards')
    expect(paths).toContain('/explore')
    expect(paths).toContain('/alerts')
    expect(paths).toContain('/datasources')
    expect(paths).toContain('/templates')
    expect(paths).toContain('/settings')
  })

  it('does not show user section when not authenticated', () => {
    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.find('.avatar').exists()).toBe(false)
    expect(wrapper.find('button').exists()).toBe(false)
  })

  it('shows user info and logout button when authenticated', () => {
    signIn({ first_name: 'Test', last_name: 'User' })

    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.find('.avatar').exists()).toBe(true)
    expect(wrapper.text()).toContain('Test')
    expect(wrapper.find('button').exists()).toBe(true)
  })

  it('shows initials when no avatar url', () => {
    signIn({ first_name: 'Alice', last_name: 'Bob' })
    const wrapper = shallowMount(AppSidebar, mountOpts)
    expect(wrapper.text()).toContain('AB')
  })
})
