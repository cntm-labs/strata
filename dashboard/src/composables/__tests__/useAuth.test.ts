import { describe, it, expect, beforeEach, vi } from 'vitest'

// `vi.mock` is hoisted above imports. Anything the mock factory references
// must be hoisted too, so we use `vi.hoisted` to lift our shared mock state
// alongside it.
const { NucleusMock, listeners, fireListeners } = vi.hoisted(() => {
  const listeners: Array<() => void> = []
  const NucleusMock = {
    user: null as { id: string; email: string } | null,
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
  return { NucleusMock, listeners, fireListeners }
})

vi.mock('@cntm-labs/nucleus-js', () => ({ Nucleus: NucleusMock }))

import { useAuth } from '../useAuth'

describe('useAuth', () => {
  beforeEach(() => {
    NucleusMock.user = null
    NucleusMock.organization = null
    NucleusMock.session = null
    NucleusMock.getToken.mockClear()
    NucleusMock.signOut.mockClear()
    fireListeners()
  })

  it('reflects signed-out state when Nucleus is unsigned', () => {
    const { isAuthenticated, user } = useAuth()
    expect(isAuthenticated.value).toBe(false)
    expect(user.value).toBeNull()
  })

  it('reflects signed-in state after Nucleus signs in', () => {
    const { isAuthenticated, user } = useAuth()
    expect(isAuthenticated.value).toBe(false)

    NucleusMock.user = { id: 'u-1', email: 'alice@test.com' }
    NucleusMock.session = { token: 'jwt-1' }
    fireListeners()

    expect(isAuthenticated.value).toBe(true)
    expect(user.value).toEqual({ id: 'u-1', email: 'alice@test.com' })
  })

  it('reflects organization changes', () => {
    const { organization } = useAuth()
    expect(organization.value).toBeNull()

    NucleusMock.organization = { id: 'org-1', name: 'Acme', slug: 'acme' }
    fireListeners()

    expect(organization.value).toEqual({ id: 'org-1', name: 'Acme', slug: 'acme' })
  })

  it('getToken proxies Nucleus.getToken', () => {
    NucleusMock.session = { token: 'jwt-xyz' }
    const { getToken } = useAuth()
    expect(getToken()).toBe('jwt-xyz')
    expect(NucleusMock.getToken).toHaveBeenCalled()
  })

  it('signOut calls Nucleus.signOut and updates reactive state', async () => {
    NucleusMock.user = { id: 'u-1', email: 'alice@test.com' }
    NucleusMock.session = { token: 'jwt-1' }
    fireListeners()

    const { isAuthenticated, signOut } = useAuth()
    expect(isAuthenticated.value).toBe(true)

    await signOut()

    expect(NucleusMock.signOut).toHaveBeenCalled()
    expect(isAuthenticated.value).toBe(false)
  })

  it('shares state across multiple useAuth() calls (singleton subscription)', () => {
    const auth1 = useAuth()
    const auth2 = useAuth()

    NucleusMock.user = { id: 'u-1', email: 'shared@test.com' }
    NucleusMock.session = { token: 'jwt-1' }
    fireListeners()

    expect(auth1.isAuthenticated.value).toBe(true)
    expect(auth2.isAuthenticated.value).toBe(true)
    expect(auth2.user.value?.email).toBe('shared@test.com')
  })

  it('registers exactly one listener even with many useAuth() calls', () => {
    const initial = listeners.length
    useAuth()
    useAuth()
    useAuth()
    // ensureSubscribed runs at most once across the module's lifetime.
    expect(listeners.length).toBe(initial)
  })
})
