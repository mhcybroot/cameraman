<script setup>
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'

const currentModule = ref('dashboard')

// Auth State
const authToken = ref(localStorage.getItem('cameraman_token') || '')
const authUser = ref(localStorage.getItem('cameraman_username') || '')
const authRole = ref(localStorage.getItem('cameraman_role') || '')

const loginForm = ref({
  username: '',
  password: ''
})
const loginError = ref('')
const isLoggingIn = ref(false)

const handleLogin = async () => {
  if (!loginForm.value.username.trim() || !loginForm.value.password.trim()) {
    loginError.value = 'Username and password are required'
    return
  }
  loginError.value = ''
  isLoggingIn.value = true
  try {
    const response = await fetch('/api/auth/login', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(loginForm.value)
    })
    if (response.ok) {
      const data = await response.json()
      authToken.value = data.token
      authUser.value = data.username
      authRole.value = data.role
      
      localStorage.setItem('cameraman_token', data.token)
      localStorage.setItem('cameraman_username', data.username)
      localStorage.setItem('cameraman_role', data.role)
      
      loginForm.value = { username: '', password: '' }
      
      // Load data
      await fetchEvents()
      await fetchCameras()
      await fetchAiConfigs()
      if (authRole.value === 'admin') {
        await fetchUsers()
      }
      
      currentModule.value = 'dashboard'
    } else {
      const err = await response.json()
      loginError.value = err.message || 'Login failed'
    }
  } catch (error) {
    loginError.value = 'Connection error logging in'
    console.error(error)
  } finally {
    isLoggingIn.value = false
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

const handleLogout = async () => {
  try {
    await fetch('/api/auth/logout', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${authToken.value}`
      }
    })
  } catch (e) {
    console.error('Logout error:', e)
  }
  
  authToken.value = ''
  authUser.value = ''
  authRole.value = ''
  localStorage.removeItem('cameraman_token')
  localStorage.removeItem('cameraman_username')
  localStorage.removeItem('cameraman_role')
  
  events.value = []
  cameras.value = []
  aiConfigs.value = []
  users.value = []
  currentModule.value = 'dashboard'
  
  nextTick(() => {
    if (window.lucide) {
      window.lucide.createIcons()
    }
  })
}

// Authenticated Fetch Wrapper
const authFetch = async (url, options = {}) => {
  const headers = options.headers || {}
  if (authToken.value) {
    headers['Authorization'] = `Bearer ${authToken.value}`
  }
  const response = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...headers
    }
  })
  if (response.status === 401 && url !== '/api/auth/login') {
    handleLogout()
  }
  return response
}

const events = ref([])
const selectedEvent = ref(null)
const autoRefresh = ref(true)
const refreshCountdown = ref(3)
let refreshInterval = null

const filter = ref({
  search: '',
  status: 'all',
  district: 'all'
})

const cameraFilter = ref('all')

const systemLogs = ref([
  { timestamp: new Date().toLocaleTimeString(), type: 'info', message: 'System Diagnostics Portal initialized.' },
  { timestamp: new Date().toLocaleTimeString(), type: 'info', message: 'CCTV Webhook listener online at http://localhost:8080/api/webhooks/camera' }
])

const addLog = (type, message) => {
  systemLogs.value.unshift({
    timestamp: new Date().toLocaleTimeString(),
    type,
    message
  })
  if (systemLogs.value.length > 50) {
    systemLogs.value.pop()
  }
}

// Watch events to append logs dynamically when a new event arrives
watch(events, (newVal, oldVal) => {
  if (!oldVal || oldVal.length === 0) return
  const oldIds = new Set(oldVal.map(e => e.id))
  newVal.forEach(e => {
    if (!oldIds.has(e.id)) {
      addLog('success', `[Webhook Ingest] Image received from camera ${e.camera_id} - Saved as ${e.image_path}`)
      addLog('info', `[AI OCR] Vision Model Output: "${e.raw_ai_text ? e.raw_ai_text.replace(/\n/g, ' ') : 'None'}"`)
      addLog(
        e.is_plate_valid ? 'success' : 'warning', 
        `[Validation] Plate ${e.is_plate_valid ? 'VALIDATED' : 'FAILED'} - District: ${e.district || 'N/A'}, Class: ${e.vehicle_class || 'N/A'}, Plate: ${e.plate_number || 'N/A'}`
      )
    }
  })
}, { deep: true })

// Fetch database records
const fetchEvents = async () => {
  if (!authToken.value) return
  try {
    const response = await authFetch('/api/events')
    if (response.ok) {
      const data = await response.json()
      events.value = data
      
      // Keep selection if it exists and is still in list
      if (selectedEvent.value) {
        const found = data.find(e => e.id === selectedEvent.value.id)
        if (found) {
          selectedEvent.value = found
        }
      }
    }
  } catch (error) {
    console.error('Failed to fetch events:', error)
  } finally {
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

// Compute overall statistics
const stats = computed(() => {
  const total = events.value.length
  const valid = events.value.filter(e => e.is_plate_valid).length
  const rate = total > 0 ? Math.round((valid / total) * 100) : 0
  return { total, valid, rate }
})

// Districts list
const districts = computed(() => {
  const districtsSet = new Set()
  events.value.forEach(e => {
    if (e.district) {
      districtsSet.add(e.district)
    }
  })
  return Array.from(districtsSet).sort()
})

// Filter logic
const filteredEvents = computed(() => {
  return events.value.filter(event => {
    if (cameraFilter.value !== 'all' && event.camera_id !== cameraFilter.value) {
      return false
    }
    const searchLower = filter.value.search.trim().toLowerCase()
    if (searchLower) {
      const num = event.plate_number || ''
      if (!num.toLowerCase().includes(searchLower)) {
        return false
      }
    }
    if (filter.value.status === 'valid' && !event.is_plate_valid) {
      return false
    }
    if (filter.value.status === 'invalid' && event.is_plate_valid) {
      return false
    }
    if (filter.value.district !== 'all' && event.district !== filter.value.district) {
      return false
    }
    return true
  })
})

// Dashboard breakdowns
const districtBreakdown = computed(() => {
  const counts = {}
  events.value.forEach(e => {
    if (e.district) {
      counts[e.district] = (counts[e.district] || 0) + 1
    }
  })
  return Object.entries(counts)
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count)
})

const classBreakdown = computed(() => {
  const counts = {}
  events.value.forEach(e => {
    if (e.vehicle_class) {
      counts[e.vehicle_class] = (counts[e.vehicle_class] || 0) + 1
    }
  })
  return Object.entries(counts)
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count)
})

const recentEvents = computed(() => {
  return events.value.slice(0, 5)
})

const selectEvent = (event) => {
  selectedEvent.value = event
  nextTick(() => {
    if (window.lucide) {
      window.lucide.createIcons()
    }
  })
}

const formatTime = (timeStr) => {
  if (!timeStr) return ''
  const d = new Date(timeStr)
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

const formatFullDateTime = (timeStr) => {
  if (!timeStr) return ''
  const d = new Date(timeStr)
  return d.toLocaleString()
}

// Handle refresh interval
const startTimer = () => {
  refreshCountdown.value = 3
  refreshInterval = setInterval(async () => {
    if (autoRefresh.value && authToken.value) {
      refreshCountdown.value--
      if (refreshCountdown.value <= 0) {
        await fetchEvents()
        refreshCountdown.value = 3
      }
    }
  }, 1000)
}

const triggerManualRefresh = async () => {
  if (!authToken.value) return
  addLog('info', 'Refreshing system data...')
  await fetchEvents()
  await fetchCameras()
  await fetchAiConfigs()
  if (authRole.value === 'admin') {
    await fetchUsers()
  }
  addLog('success', 'System data refreshed successfully.')
}

const cameras = ref([])
const newCamera = ref({
  id: '',
  name: '',
  location: ''
})
const addCameraError = ref('')
const isAddingCamera = ref(false)

const fetchCameras = async () => {
  if (!authToken.value) return
  try {
    const response = await authFetch('/api/cameras')
    if (response.ok) {
      cameras.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to fetch cameras:', error)
  }
}

const addCamera = async () => {
  if (authRole.value !== 'admin') return
  if (!newCamera.value.name.trim()) {
    addCameraError.value = 'Camera Name is required'
    return
  }
  addCameraError.value = ''
  isAddingCamera.value = true
  try {
    const response = await authFetch('/api/cameras', {
      method: 'POST',
      body: JSON.stringify({
        id: newCamera.value.id.trim() || undefined,
        name: newCamera.value.name.trim(),
        location: newCamera.value.location.trim() || undefined
      })
    })
    if (response.ok) {
      addLog('success', `Registered Camera: "${newCamera.value.name}" (${newCamera.value.id || 'auto-id'})`)
      newCamera.value = { id: '', name: '', location: '' }
      await fetchCameras()
    } else {
      const err = await response.json()
      addCameraError.value = err.message || 'Failed to add camera'
    }
  } catch (error) {
    addCameraError.value = 'Network error adding camera'
    console.error(error)
  } finally {
    isAddingCamera.value = false
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

const deleteCamera = async (id) => {
  if (authRole.value !== 'admin') return
  if (!confirm(`Are you sure you want to unregister camera ${id}?`)) {
    return
  }
  try {
    const response = await authFetch(`/api/cameras/${id}`, {
      method: 'DELETE'
    })
    if (response.ok) {
      addLog('info', `Unregistered Camera: ${id}`)
      await fetchCameras()
    }
  } catch (error) {
    console.error('Failed to delete camera:', error)
  }
}

const getCameraName = (id) => {
  if (!id) return 'Unknown Cam'
  const cam = cameras.value.find(c => c.id === id)
  return cam ? cam.name : id
}

const getCameraLocation = (id) => {
  if (!id) return null
  const cam = cameras.value.find(c => c.id === id)
  return cam ? cam.location : null
}

const aiConfigs = ref([])
const newAiConfig = ref({
  name: '',
  provider_type: 'gemini',
  api_key: '',
  model_name: 'gemini-2.5-flash',
  is_active: false
})
const addAiConfigError = ref('')
const isAddingAiConfig = ref(false)

watch(() => newAiConfig.value.provider_type, (newType) => {
  if (newType === 'gemini') {
    newAiConfig.value.model_name = 'gemini-2.5-flash'
  } else if (newType === 'minimax') {
    newAiConfig.value.model_name = 'MiniMax-M3'
  } else {
    newAiConfig.value.model_name = ''
  }
})

const fetchAiConfigs = async () => {
  if (!authToken.value) return
  try {
    const response = await authFetch('/api/ai-configs')
    if (response.ok) {
      aiConfigs.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to fetch AI configurations:', error)
  }
}

const activeAiConfig = computed(() => {
  return aiConfigs.value.find(c => c.is_active) || null
})

const addAiConfig = async () => {
  if (authRole.value !== 'admin') return
  if (!newAiConfig.value.name.trim()) {
    addAiConfigError.value = 'Configuration Name is required'
    return
  }
  const isKeyRequired = newAiConfig.value.provider_type === 'gemini' || newAiConfig.value.provider_type === 'minimax'
  if (isKeyRequired && !newAiConfig.value.api_key.trim()) {
    addAiConfigError.value = `API Key is required for ${newAiConfig.value.provider_type === 'gemini' ? 'Gemini' : 'Minimax'}`
    return
  }

  addAiConfigError.value = ''
  isAddingAiConfig.value = true
  try {
    const response = await authFetch('/api/ai-configs', {
      method: 'POST',
      body: JSON.stringify({
        name: newAiConfig.value.name.trim(),
        provider_type: newAiConfig.value.provider_type,
        api_key: isKeyRequired ? newAiConfig.value.api_key.trim() : undefined,
        model_name: newAiConfig.value.model_name.trim() || undefined,
        is_active: newAiConfig.value.is_active
      })
    })

    if (response.ok) {
      addLog('success', `Created AI Config: "${newAiConfig.value.name}"`)
      newAiConfig.value = {
        name: '',
        provider_type: 'gemini',
        api_key: '',
        model_name: 'gemini-2.5-flash',
        is_active: false
      }
      await fetchAiConfigs()
    } else {
      const err = await response.json()
      addAiConfigError.value = err.message || 'Failed to save configuration'
    }
  } catch (error) {
    addAiConfigError.value = 'Network error saving configuration'
    console.error(error)
  } finally {
    isAddingAiConfig.value = false
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

const deleteAiConfig = async (id) => {
  if (authRole.value !== 'admin') return
  if (!confirm('Are you sure you want to delete this AI configuration?')) {
    return
  }
  try {
    const response = await authFetch(`/api/ai-configs/${id}`, {
      method: 'DELETE'
    })
    if (response.ok) {
      addLog('info', `Deleted AI Config: ${id}`)
      await fetchAiConfigs()
    }
  } catch (error) {
    console.error('Failed to delete AI configuration:', error)
  }
}

const activateAiConfig = async (id) => {
  if (authRole.value !== 'admin') return
  try {
    const response = await authFetch(`/api/ai-configs/${id}/activate`, {
      method: 'POST'
    })
    if (response.ok) {
      const config = aiConfigs.value.find(c => c.id === id)
      addLog('success', `Activated AI Config: "${config ? config.name : id}"`)
      await fetchAiConfigs()
    }
  } catch (error) {
    console.error('Failed to activate AI configuration:', error)
  }
}

// User Management State & Actions
const users = ref([])
const newUser = ref({
  username: '',
  password: '',
  role: 'user'
})
const addUserError = ref('')
const isAddingUser = ref(false)

const fetchUsers = async () => {
  if (!authToken.value || authRole.value !== 'admin') return
  try {
    const response = await authFetch('/api/users')
    if (response.ok) {
      users.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to fetch users:', error)
  }
}

const addUser = async () => {
  if (authRole.value !== 'admin') return
  if (!newUser.value.username.trim()) {
    addUserError.value = 'Username is required'
    return
  }
  addUserError.value = ''
  isAddingUser.value = true
  try {
    const response = await authFetch('/api/users', {
      method: 'POST',
      body: JSON.stringify({
        username: newUser.value.username.trim(),
        password: newUser.value.password.trim() || undefined,
        role: newUser.value.role
      })
    })
    if (response.ok) {
      addLog('success', `Created User: "${newUser.value.username}"`)
      newUser.value = { username: '', password: '', role: 'user' }
      await fetchUsers()
    } else {
      const err = await response.json()
      addUserError.value = err.message || 'Failed to create user'
    }
  } catch (error) {
    addUserError.value = 'Network error creating user'
  } finally {
    isAddingUser.value = false
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
}

const toggleBlockUser = async (id) => {
  if (authRole.value !== 'admin') return
  try {
    const response = await authFetch(`/api/users/${id}/toggle-block`, {
      method: 'POST'
    })
    if (response.ok) {
      addLog('info', `Toggled block state on user profile: ${id}`)
      await fetchUsers()
    }
  } catch (error) {
    console.error('Failed to toggle block status:', error)
  }
}

const resetUserPassword = async (id) => {
  if (authRole.value !== 'admin') return
  const newPass = prompt('Enter the new password for this user:')
  if (newPass === null) return
  if (!newPass.trim()) {
    alert('Password cannot be empty')
    return
  }
  try {
    const response = await authFetch(`/api/users/${id}/reset-password`, {
      method: 'POST',
      body: JSON.stringify({ new_password: newPass.trim() })
    })
    if (response.ok) {
      alert('Password updated successfully')
      addLog('success', `Reset password for user profile: ${id}`)
    } else {
      const err = await response.json()
      alert(err.message || 'Failed to reset password')
    }
  } catch (error) {
    console.error('Failed to reset password:', error)
  }
}

watch(autoRefresh, (newVal) => {
  if (newVal) {
    refreshCountdown.value = 3
  }
})

watch(currentModule, () => {
  nextTick(() => {
    if (window.lucide) {
      window.lucide.createIcons()
    }
  })
})

const triggerSimulatedLog = async () => {
  if (authRole.value !== 'admin') return
  addLog('info', '[Simulation] Dispatching mock POST /api/webhooks/camera payload...')
  try {
    const response = await fetch('/api/webhooks/camera', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        camera_id: 'simulated-test-cam',
        image: 'data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wCEAAkGBwgHBgkIBwgKCgkLDRYPDQwMDRsUFRAWIB0iIiAdHx8kKDQsJCYxJx8fLT0tMTU3Ojo6Iys/RD84QzQ5OjcBCgoKDQwNGg8PGjclHyU3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3N//AABEIAKAA8AMBIgACEQEDEQH/xAAbAAACAwEBAQAAAAAAAAAAAAACBAMFBgEAB//EAEcQAAEDAwIDBAcDCAgGAwEAAAECAwQABREGIRIxQRNRYXEUIjKBkaGxM0JSBxUjQ2JygsEkY3OywtHh8CY2U2R0oiVUkhb/xAAWAQEBAQAAAAAAAAAAAAAAAAAAAQL/xAAaEQEBAQEAAwAAAAAAAAAAAAAAEQEhAhIx/9oADAMBAAIRAxEAPwB9tVMJVRR7PdHt24EjzUggfOrBrTd0OONploHq66n+RNaZIpXUiVVao004AO2nxm/3cr/yphNjgN/a3BxZ/q28fWoql4q7nar5NvszfMSHFftKx9KlSi1I9mChRHVaiaDN5GKJsKcVhpKlnuQMn5VphMjt/YxGEEdQ2M0Srs+RhK8DuAxQUKLfNc9iFJJ8WiB8TU6LHcifWYDY/bcSP51YLnPK3UtR99RmQs9TQQCwv/rZEVH8ZV/KjFkaH2lwRj9hk/zou0V31ziUepqBeXp+2SGy27LlKB6ABNYrUUD0KWW05WwrPZlac5SdiD31vfXPSk7hbUz2FNOp58iOYqbi5rO6bvyXEiJM4WXE7JyRwq8j3VfSR2jSkYzxDFZC46dnw1FTbRcQDlK29yD5cwfKvRLi+wkNPKcTw/iSVfTf41F1XPvyLRJbSw2FodkdktvhPEonlg5wPM7bVdwV+k6mW3HQpqHBYDbSchWSoDfiHM47qB163yM9ulS1EDJ9YYI6jbY03AfdShTVrgOOur3KylSifMmmDStssMR1S7i6GY+ThP33D3AVQ2qOqXcF3AhYjI4kxG1nPDxH1leJPLPdVjD01NmLS/opt9gDUnp3E93gK0CIbLKQFKbbSNgMjArSKzBPQ1zs1E0+uRbWt1ym/cc/So13WCkZbbedHels4+JoFfR1nofhReiKxQKvzPEUMst5xkhboyPcMml5N7fbSFK4UhXLs2lK+ZxTga9DJ5170MDc7DxrPr1IlxRQHnjvuDt8Bj6HNJSLq44f0cSQ9+2scH8fxSjTu+iN+2+2D+9mllPxv1bbjmeXCg1nhcriB+hYiMZ+8fXV/l86VfeuDiiXrq+gHmlk8AB/xVyq9ST7VDeoqD9k99UUTMd1A+9t5mh5UEapbpGxT7qEUWaEUWa6K6A0KFFRAoq6K4B51wDvoPUPMiu14CvdcV7rqh199cqGXLZjNlTquEc9+tVMm8SHeL0Nrs2x+sWNyPAdKiLkuISrBXg92K56S1nAdv4ZrPelyXUkuyHFd+5Apu0w5dzWGbe0tzqpfEQkeexqC4akNL2Us/EU6hhDnspz/FmpeHpDTdsR2t1eXNXnKmWDwp96tvkatXNX2SzthqyWqIyRyU4OJRHvTmirOPpK8SclqO0hH9e4UfyP0q0R+TqasfnGfFjJ7ktlzHzIrGydY3aWT2sxQHe2so+hqsXdnHTlx1TniVnPzqi+un5PpTRH5uucaVk/p1lg48OKqCT+Tu/tH1HbehPcuXg/SmGLy63jhfWn97hpyLqaajZUhRA6FXFQS2jRNnt4C9QSJdzf5lpt4oaB8Ukbj31o2XosJrostP2uFbmsc0tDjV7ynFJQr/FlrCZLWD+NPUVZMtsPp4mF8We/pSCz6pbiuJ24k9y1kj4GgVc34FhC1fvKzRer51wGgQelvu/aLakaT1HdLhGfCjlnnkeu/nVhG0lqG4gKIMK1sNfvuHf4CrCF+Ty2RQDIBPihvCrm36dtFub7OC0gHqptPBn/FQSx20MMIYbPrNkI9b3k0Pq95piRbYsJrua7/AFiOD+QqBDr6z67mB4GglU22nctpPiqpEONp5JSPEUtwt9yye8mq+XObbB4UJ+FBcelsN/eGfAUnLujSc9mCD5'
      })
    })
    
    if (response.ok) {
      addLog('success', '[Simulation] Webhook request accepted by server.')
      setTimeout(async () => {
        await fetchEvents()
        addLog('success', '[Simulation] Events refreshed. New event added!')
      }, 1500)
    } else {
      addLog('warning', `[Simulation] Webhook request returned status ${response.status}`)
    }
  } catch (error) {
    addLog('warning', `[Simulation] Network error sending simulated webhook: ${error}`)
  }
}

const getModuleTitle = () => {
  switch (currentModule.value) {
    case 'dashboard':
      return 'Surveillance Dashboard'
    case 'events':
      return 'CCTV Activity Log'
    case 'cameras':
      return 'Camera Registry'
    case 'ai':
      return 'AI Configs Manager'
    case 'users':
      return 'Users Administration'
    case 'diagnostics':
      return 'Diagnostics Console'
    default:
      return 'System Portal'
  }
}

const getModuleSubtitle = () => {
  switch (currentModule.value) {
    case 'dashboard':
      return 'Real-time statistics and traffic monitoring analytics'
    case 'events':
      return 'All processed plate detection events and OCR telemetry'
    case 'cameras':
      return 'Manage registered CCTV stream feeds and ingestion hooks'
    case 'ai':
      return 'Manage and switch active Vision API models at runtime'
    case 'users':
      return 'Manage portal users, reset passwords, and toggle blocks'
    case 'diagnostics':
      return 'Simulate camera triggers and monitor real-time server diagnostics logs'
    default:
      return 'CCTV AI License Plate Processor'
  }
}

onMounted(async () => {
  if (authToken.value) {
    await fetchEvents()
    await fetchCameras()
    await fetchAiConfigs()
    if (authRole.value === 'admin') {
      await fetchUsers()
    }
    startTimer()
  } else {
    nextTick(() => {
      if (window.lucide) {
        window.lucide.createIcons()
      }
    })
  }
})

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval)
  }
})
</script>

<template>
  <!-- 1. LOGIN SCREEN WALL (shown if missing token) -->
  <div v-if="!authToken" class="login-overlay">
    <div class="login-card">
      <div class="login-brand">
        <div class="logo-icon">
          <i data-lucide="eye" style="color: white; width: 22px; height: 22px;"></i>
        </div>
        <span class="brand-text">CAMERAMAN LOGIN</span>
      </div>
      <p class="login-subtitle">Access the AI CCTV surveillance portal</p>
      
      <form @submit.prevent="handleLogin" class="login-form-fields">
        <div class="form-group">
          <label>Username</label>
          <input type="text" placeholder="Enter username" v-model="loginForm.username" required>
        </div>
        <div class="form-group">
          <label>Password</label>
          <input type="password" placeholder="Enter password" v-model="loginForm.password" required>
        </div>
        
        <div v-if="loginError" class="form-error">
          {{ loginError }}
        </div>
        
        <button type="submit" class="submit-btn w-full" :disabled="isLoggingIn">
          <i data-lucide="log-in" style="width: 16px; height: 16px; display: inline-block; vertical-align: middle; margin-right: 4px;"></i>
          {{ isLoggingIn ? 'Logging in...' : 'Sign In' }}
        </button>
      </form>
    </div>
  </div>

  <!-- 2. MAIN APPLICATION WORKSPACE -->
  <div v-else class="app-layout">
    <!-- Left Navigation Bar -->
    <aside class="sidebar-nav">
      <div class="nav-brand">
        <div class="logo-icon">
          <i data-lucide="eye" style="color: white; width: 20px; height: 20px;"></i>
        </div>
        <span class="brand-text">CAMERAMAN</span>
      </div>

      <div class="nav-menu">
        <button 
          class="nav-item" 
          :class="{ active: currentModule === 'dashboard' }" 
          @click="currentModule = 'dashboard'"
        >
          <i data-lucide="layout-dashboard"></i>
          <span>Dashboard</span>
        </button>

        <button 
          class="nav-item" 
          :class="{ active: currentModule === 'events' }" 
          @click="currentModule = 'events'"
        >
          <i data-lucide="activity"></i>
          <span>Events Feed</span>
        </button>

        <button 
          class="nav-item" 
          :class="{ active: currentModule === 'cameras' }" 
          @click="currentModule = 'cameras'"
        >
          <i data-lucide="video"></i>
          <span>Cameras</span>
        </button>

        <button 
          class="nav-item" 
          :class="{ active: currentModule === 'ai' }" 
          @click="currentModule = 'ai'"
        >
          <i data-lucide="cpu"></i>
          <span>AI Settings</span>
        </button>

        <button 
          v-if="authRole === 'admin'"
          class="nav-item" 
          :class="{ active: currentModule === 'users' }" 
          @click="currentModule = 'users'"
        >
          <i data-lucide="users"></i>
          <span>Users</span>
        </button>

        <button 
          class="nav-item" 
          :class="{ active: currentModule === 'diagnostics' }" 
          @click="currentModule = 'diagnostics'"
        >
          <i data-lucide="terminal"></i>
          <span>Diagnostics</span>
        </button>
      </div>

      <div class="nav-footer">
        <div class="user-profile-card">
          <div class="user-profile-info">
            <div class="profile-avatar">
              <i data-lucide="user" style="width: 14px; height: 14px; color: white;"></i>
            </div>
            <div class="profile-text" style="display: flex; flex-direction: column; align-items: flex-start; gap: 0.15rem;">
              <div class="profile-username">{{ authUser }}</div>
              <span class="badge-role" :class="authRole" style="font-size: 0.55rem; padding: 0.05rem 0.25rem;">
                {{ authRole }}
              </span>
            </div>
          </div>
          <button class="logout-btn" @click="handleLogout" title="Sign Out">
            <i data-lucide="log-out" style="width: 14px; height: 14px;"></i>
          </button>
        </div>
      </div>
    </aside>

    <!-- Main Content Workspace -->
    <main class="main-content">
      <!-- Header -->
      <header class="content-header">
        <div class="header-text">
          <h1 class="page-title">{{ getModuleTitle() }}</h1>
          <p class="page-subtitle">{{ getModuleSubtitle() }}</p>
        </div>

        <div class="header-actions">
          <div class="polling-status">
            <span :class="autoRefresh ? 'pulse-amber' : 'pulse-gray'"></span>
            <span class="status-text">
              {{ autoRefresh ? `Auto-refresh in ${refreshCountdown}s` : 'Polling paused' }}
            </span>
          </div>
          <button class="icon-btn" @click="autoRefresh = !autoRefresh" title="Toggle Auto Refresh">
            <i :data-lucide="autoRefresh ? 'pause' : 'play'" style="width: 16px; height: 16px;"></i>
          </button>
          <button class="icon-btn" @click="triggerManualRefresh" title="Refresh Data">
            <i data-lucide="refresh-cw" style="width: 16px; height: 16px;"></i>
          </button>
        </div>
      </header>

      <!-- Viewport -->
      <div class="workspace-viewport">
        <!-- ==================== DASHBOARD MODULE ==================== -->
        <div v-if="currentModule === 'dashboard'" class="module-view dashboard-view">
          <div class="stats-grid">
            <div class="dashboard-stat-card">
              <div class="stat-icon-wrapper blue">
                <i data-lucide="activity"></i>
              </div>
              <div class="stat-content">
                <div class="stat-label">Total Events</div>
                <div class="stat-value">{{ stats.total }}</div>
              </div>
            </div>
            <div class="dashboard-stat-card">
              <div class="stat-icon-wrapper green">
                <i data-lucide="check-circle-2"></i>
              </div>
              <div class="stat-content">
                <div class="stat-label">Valid Plates</div>
                <div class="stat-value text-green">{{ stats.valid }}</div>
              </div>
            </div>
            <div class="dashboard-stat-card">
              <div class="stat-icon-wrapper amber">
                <i data-lucide="shield-check"></i>
              </div>
              <div class="stat-content">
                <div class="stat-label">Accuracy Rate</div>
                <div class="stat-value text-amber">{{ stats.rate }}%</div>
              </div>
            </div>
            <div class="dashboard-stat-card">
              <div class="stat-icon-wrapper purple">
                <i data-lucide="video"></i>
              </div>
              <div class="stat-content">
                <div class="stat-label">Active Cameras</div>
                <div class="stat-value text-purple">{{ cameras.length }}</div>
              </div>
            </div>
          </div>

          <div class="dashboard-details-grid">
            <!-- Left Side: Recent Activity -->
            <div class="dashboard-panel recent-activity">
              <h2 class="panel-title">
                <i data-lucide="history" class="panel-title-icon"></i>
                Recent Recognition Events
              </h2>
              <div v-if="recentEvents.length === 0" class="panel-empty">
                <i data-lucide="image-off"></i>
                <p>No processed events yet.</p>
              </div>
              <div v-else class="recent-events-list">
                <div 
                  v-for="event in recentEvents" 
                  :key="event.id" 
                  class="recent-event-item"
                  @click="selectEvent(event)"
                >
                  <div class="recent-event-thumb">
                    <img :src="'/uploads/' + event.image_path">
                  </div>
                  <div class="recent-event-details">
                    <div class="recent-event-header">
                      <span class="event-cam">{{ getCameraName(event.camera_id) }}</span>
                      <span class="event-time">{{ formatTime(event.created_at) }}</span>
                    </div>
                    <div class="recent-event-plate">
                      <span v-if="event.is_plate_valid" class="event-plate-text">
                        {{ event.district }} {{ event.metro_prefix ? 'মেট্রো' : '' }} {{ event.vehicle_class }} {{ event.plate_number }}
                      </span>
                      <span v-else class="event-plate-invalid">Invalid Plate Structure</span>
                    </div>
                  </div>
                  <div class="recent-event-badge">
                    <span class="badge-dot" :class="event.is_plate_valid ? 'bg-green' : 'bg-red'"></span>
                    <span class="badge-text" :class="event.is_plate_valid ? 'text-green' : 'text-red'">
                      {{ event.is_plate_valid ? 'Valid' : 'Failed' }}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Right Side: Analytics & Model Info -->
            <div class="dashboard-panel system-breakdowns">
              <!-- Active AI Info -->
              <div class="active-ai-card">
                <h3 class="active-ai-title">
                  <i data-lucide="cpu" style="width: 18px; height: 18px; color: var(--accent-primary);"></i>
                  Active AI Model Routing
                </h3>
                <div class="active-ai-details" v-if="activeAiConfig">
                  <div class="ai-param">
                    <span class="param-lbl">Config Name:</span>
                    <span class="param-val">{{ activeAiConfig.name }}</span>
                  </div>
                  <div class="ai-param">
                    <span class="param-lbl">Provider:</span>
                    <span class="param-val text-capitalize">{{ activeAiConfig.provider_type }}</span>
                  </div>
                  <div class="ai-param">
                    <span class="param-lbl">Model:</span>
                    <span class="param-val font-mono">{{ activeAiConfig.model_name || 'default' }}</span>
                  </div>
                </div>
                <div class="active-ai-details" v-else>
                  <div class="ai-param">
                    <span class="param-lbl">Status:</span>
                    <span class="param-val text-amber">Using environment default fallback</span>
                  </div>
                </div>
              </div>

              <!-- District Breakdown -->
              <div class="breakdown-section">
                <h3 class="breakdown-title">District Distribution</h3>
                <div v-if="districtBreakdown.length === 0" class="breakdown-empty">
                  No district data available.
                </div>
                <div v-else class="breakdown-list">
                  <div v-for="dist in districtBreakdown.slice(0, 4)" :key="dist.name" class="breakdown-item">
                    <div class="breakdown-bar-lbl">
                      <span>{{ dist.name }}</span>
                      <span>{{ dist.count }}</span>
                    </div>
                    <div class="breakdown-bar-bg">
                      <div class="breakdown-bar-fill" :style="{ width: (dist.count / stats.total * 100) + '%' }"></div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Class Breakdown -->
              <div class="breakdown-section">
                <h3 class="breakdown-title">Vehicle Class Breakdown</h3>
                <div v-if="classBreakdown.length === 0" class="breakdown-empty">
                  No vehicle class data.
                </div>
                <div class="class-badges-grid">
                  <div v-for="cls in classBreakdown.slice(0, 8)" :key="cls.name" class="class-badge-item">
                    <span class="class-letter">{{ cls.name }}</span>
                    <span class="class-count">{{ cls.count }}</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ==================== EVENTS FEED MODULE ==================== -->
        <div v-else-if="currentModule === 'events'" class="module-view events-view">
          <!-- Filters Toolbar -->
          <div class="events-toolbar">
            <div class="toolbar-search">
              <i data-lucide="search"></i>
              <input type="text" placeholder="Search license plate digits..." v-model="filter.search">
            </div>

            <div class="toolbar-filters">
              <!-- Camera Filter -->
              <div class="filter-group-item">
                <label>Camera</label>
                <select class="form-select" v-model="cameraFilter">
                  <option value="all">All Cameras</option>
                  <option v-for="cam in cameras" :key="cam.id" :value="cam.id">
                    {{ cam.name }}
                  </option>
                </select>
              </div>

              <!-- District Filter -->
              <div class="filter-group-item">
                <label>District</label>
                <select class="form-select" v-model="filter.district">
                  <option value="all">All Districts</option>
                  <option v-for="dist in districts" :key="dist" :value="dist">
                    {{ dist }}
                  </option>
                </select>
              </div>

              <!-- Status Filters -->
              <div class="filter-group-item">
                <label>Validation</label>
                <div class="btn-group">
                  <button 
                    class="btn-group-btn" 
                    :class="{ active: filter.status === 'all' }" 
                    @click="filter.status = 'all'"
                  >All</button>
                  <button 
                    class="btn-group-btn" 
                    :class="{ active: filter.status === 'valid' }" 
                    @click="filter.status = 'valid'"
                  >Valid</button>
                  <button 
                    class="btn-group-btn" 
                    :class="{ active: filter.status === 'invalid' }" 
                    @click="filter.status = 'invalid'"
                  >Failed</button>
                </div>
              </div>
            </div>
          </div>

          <!-- Events Grid List -->
          <div v-if="filteredEvents.length === 0" class="events-empty-state">
            <i data-lucide="alert-circle" style="width: 48px; height: 48px; color: var(--text-muted);"></i>
            <h3>No events match filters</h3>
            <p>Try adjusting your search query, camera filter, or validation status.</p>
          </div>
          <div v-else class="events-grid-container">
            <div 
              v-for="event in filteredEvents" 
              :key="event.id" 
              class="event-grid-card"
              :class="{ 'border-green-glow': event.is_plate_valid, 'border-red-glow': !event.is_plate_valid }"
              @click="selectEvent(event)"
            >
              <div class="event-card-img">
                <img :src="'/uploads/' + event.image_path">
                <span class="event-card-time">{{ formatTime(event.created_at) }}</span>
              </div>
              <div class="event-card-content">
                <div class="event-card-meta">
                  <span class="cam-lbl">
                    <i data-lucide="video" style="width: 12px; height: 12px; display: inline-block; vertical-align: middle; margin-right: 2px;"></i>
                    {{ getCameraName(event.camera_id) }}
                  </span>
                </div>
                <div class="event-card-plate">
                  <div v-if="event.is_plate_valid" class="card-plate-rendered">
                    <div class="card-plate-top">{{ event.district }} {{ event.metro_prefix ? 'মেট্রো' : '' }} {{ event.vehicle_class }}</div>
                    <div class="card-plate-bottom">{{ event.plate_number }}</div>
                  </div>
                  <div v-else class="card-plate-error">
                    <i data-lucide="alert-triangle" style="width: 14px; height: 14px; vertical-align: middle; margin-right: 4px;"></i>
                    OCR Format Invalid
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ==================== CAMERA REGISTRY MODULE ==================== -->
        <div v-else-if="currentModule === 'cameras'" class="module-view cameras-view">
          <div class="cameras-layout-grid">
            <!-- Left Side: Active Registry Cards -->
            <div class="cameras-panel">
              <h2 class="panel-title">Registered Cameras</h2>
              <div v-if="cameras.length === 0" class="panel-empty" style="min-height: 250px;">
                <i data-lucide="video-off"></i>
                <p>No registered cameras.</p>
              </div>
              <div v-else class="cameras-cards-grid">
                <div v-for="cam in cameras" :key="cam.id" class="camera-detail-card">
                  <!-- Simulated Live Stream Preview -->
                  <div class="camera-stream-box">
                    <div class="camera-scanline"></div>
                    <div class="camera-overlay-info">
                      <div class="camera-feed-dot red-pulse"></div>
                      <span class="feed-title">LIVE FEED</span>
                    </div>
                    <div class="camera-stream-fallback">
                      <i data-lucide="video" class="stream-icon-bg"></i>
                      <div class="camera-stream-data">
                        FPS: 30 | 1080p | {{ cam.id }}
                      </div>
                    </div>
                  </div>

                  <!-- Details -->
                  <div class="camera-card-text">
                    <div class="camera-row">
                      <span class="camera-name-text">{{ cam.name }}</span>
                    </div>
                    <div class="camera-row">
                      <i data-lucide="map-pin" style="width: 12px; height: 12px; color: var(--text-muted); display: inline-block; vertical-align: middle;"></i>
                      <span class="camera-loc-val">{{ cam.location || 'No Location Set' }}</span>
                    </div>
                    <div class="camera-row font-mono" style="font-size: 0.75rem; color: var(--accent-primary);">
                      ID: {{ cam.id }}
                    </div>
                  </div>

                  <!-- Actions (Admin only) -->
                  <div v-if="authRole === 'admin'" class="camera-actions-row">
                    <button class="delete-btn-card" @click="deleteCamera(cam.id)">
                      <i data-lucide="trash-2" style="width: 13px; height: 13px; display: inline-block; vertical-align: middle; margin-right: 4px;"></i>
                      Unregister Camera
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Right Side: Register Form (Admin) / Read Only Banner (User) -->
            <div class="form-panel-side">
              <div v-if="authRole === 'admin'" class="add-camera-form" style="border: none; padding-top: 0;">
                <h3 class="form-title">Register New Camera</h3>
                
                <div class="form-group">
                  <label>Camera ID *</label>
                  <input type="text" placeholder="e.g. cam-north-gate (unique slug)" v-model="newCamera.id">
                </div>

                <div class="form-group">
                  <label>Camera Name *</label>
                  <input type="text" placeholder="e.g. North Gate Camera" v-model="newCamera.name">
                </div>

                <div class="form-group">
                  <label>Location (optional)</label>
                  <input type="text" placeholder="e.g. Warehouse Entrance" v-model="newCamera.location">
                </div>

                <div v-if="addCameraError" class="form-error">
                  {{ addCameraError }}
                </div>

                <button class="submit-btn" :disabled="isAddingCamera" @click="addCamera">
                  <i data-lucide="plus-circle" style="width: 16px; height: 16px;"></i>
                  {{ isAddingCamera ? 'Registering...' : 'Register Camera' }}
                </button>
              </div>
              <div v-else class="read-only-banner" style="text-align: left;">
                <i data-lucide="shield-alert" style="color: var(--accent-primary); width: 32px; height: 32px; margin-bottom: 0.5rem;"></i>
                <h4 style="font-weight: 700; margin-bottom: 0.25rem;">View-Only Mode</h4>
                <p style="font-size: 0.8rem; color: var(--text-muted); line-height: 1.4;">
                  Standard operators cannot register or unregister cameras. Requires Administrator credentials.
                </p>
              </div>
            </div>
          </div>
        </div>

        <!-- ==================== AI SETTINGS MODULE ==================== -->
        <div v-else-if="currentModule === 'ai'" class="module-view ai-view">
          <div class="cameras-layout-grid">
            <!-- Left Side: Config Cards -->
            <div class="cameras-panel">
              <h2 class="panel-title">AI Provider Configs</h2>
              <div class="ai-configs-grid">
                <div 
                  v-for="config in aiConfigs" 
                  :key="config.id" 
                  class="ai-config-detail-card"
                  :class="{ 'active-glowing': config.is_active }"
                >
                  <div class="ai-card-header">
                    <div class="ai-card-meta">
                      <span class="ai-card-name">{{ config.name }}</span>
                      <span class="ai-provider-badge" :class="config.provider_type">
                        {{ config.provider_type }}
                      </span>
                    </div>
                    <span v-if="config.is_active" class="active-pill">
                      <span class="pulse-green"></span>
                      ACTIVE
                    </span>
                  </div>

                  <div class="ai-card-body-details">
                    <div class="ai-detail-row">
                      <span class="ai-lbl">Model:</span>
                      <span class="ai-val font-mono">{{ config.model_name || 'default' }}</span>
                    </div>
                    <div class="ai-detail-row">
                      <span class="ai-lbl">API Key:</span>
                      <span class="ai-val font-mono">{{ config.api_key || 'None' }}</span>
                    </div>
                  </div>

                  <!-- Actions (Admin only) -->
                  <div v-if="authRole === 'admin'" class="ai-card-actions">
                    <button 
                      v-if="!config.is_active" 
                      class="activate-btn" 
                      @click="activateAiConfig(config.id)"
                    >
                      <i data-lucide="power" style="width: 13px; height: 13px; display: inline-block; vertical-align: middle; margin-right: 4px;"></i>
                      Activate
                    </button>
                    <button class="delete-ai-btn" @click="deleteAiConfig(config.id)">
                      <i data-lucide="trash-2" style="width: 13px; height: 13px; display: inline-block; vertical-align: middle; margin-right: 4px;"></i>
                      Delete
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Right Side: Add Config Form (Admin) / Read Only (User) -->
            <div class="form-panel-side">
              <div v-if="authRole === 'admin'" class="add-camera-form" style="border: none; padding-top: 0;">
                <h3 class="form-title">Add AI Configuration</h3>
                
                <div class="form-group">
                  <label>Configuration Name *</label>
                  <input type="text" placeholder="e.g. Gemini Primary Key" v-model="newAiConfig.name">
                </div>

                <div class="form-group">
                  <label>Provider Type *</label>
                  <select class="filter-select w-full" v-model="newAiConfig.provider_type">
                    <option value="gemini">Gemini Vision</option>
                    <option value="minimax">Minimax AI</option>
                    <option value="mock">Local Mock Provider</option>
                  </select>
                </div>

                <div class="form-group" v-if="newAiConfig.provider_type === 'gemini' || newAiConfig.provider_type === 'minimax'">
                  <label>API Key *</label>
                  <input type="password" :placeholder="'Enter ' + (newAiConfig.provider_type === 'gemini' ? 'Gemini' : 'Minimax') + ' API key'" v-model="newAiConfig.api_key">
                </div>

                <div class="form-group">
                  <label>Model Name (optional)</label>
                  <input type="text" :placeholder="newAiConfig.provider_type === 'gemini' ? 'e.g. gemini-2.5-flash' : newAiConfig.provider_type === 'minimax' ? 'e.g. MiniMax-M3' : 'e.g. mock-model'" v-model="newAiConfig.model_name">
                </div>

                <div class="form-group" style="flex-direction: row; gap: 0.5rem; align-items: center; margin-top: 0.25rem;">
                  <input type="checkbox" id="ai-active-chk" v-model="newAiConfig.is_active">
                  <label for="ai-active-chk" style="cursor: pointer; margin-bottom: 0; font-size: 0.75rem;">Set as Active Configuration</label>
                </div>

                <div v-if="addAiConfigError" class="form-error">
                  {{ addAiConfigError }}
                </div>

                <button class="submit-btn" :disabled="isAddingAiConfig" @click="addAiConfig">
                  <i data-lucide="plus-circle" style="width: 16px; height: 16px;"></i>
                  {{ isAddingAiConfig ? 'Saving...' : 'Save AI Configuration' }}
                </button>
              </div>
              <div v-else class="read-only-banner" style="text-align: left;">
                <i data-lucide="shield-alert" style="color: var(--accent-primary); width: 32px; height: 32px; margin-bottom: 0.5rem;"></i>
                <h4 style="font-weight: 700; margin-bottom: 0.25rem;">View-Only Mode</h4>
                <p style="font-size: 0.8rem; color: var(--text-muted); line-height: 1.4;">
                  Standard operators cannot register, toggle, or delete AI configurations. Requires Administrator credentials.
                </p>
              </div>
            </div>
          </div>
        </div>

        <!-- ==================== USERS MODULE ==================== -->
        <div v-else-if="currentModule === 'users' && authRole === 'admin'" class="module-view users-view">
          <div class="cameras-layout-grid">
            <!-- Left Side: User list -->
            <div class="cameras-panel">
              <h2 class="panel-title">System Users</h2>
              <div class="users-list-table-container">
                <table class="users-table">
                  <thead>
                    <tr>
                      <th>Username</th>
                      <th>Role</th>
                      <th>Status</th>
                      <th>Created At</th>
                      <th style="text-align: right;">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="u in users" :key="u.id" :class="{ 'user-row-blocked': u.is_blocked }">
                      <td class="font-weight-bold" style="font-weight: 700;">{{ u.username }}</td>
                      <td>
                        <span class="badge-role" :class="u.role">{{ u.role }}</span>
                      </td>
                      <td>
                        <span class="status-indicator-text" :class="u.is_blocked ? 'blocked' : 'active'">
                          {{ u.is_blocked ? 'Blocked' : 'Active' }}
                        </span>
                      </td>
                      <td class="font-mono" style="font-size: 0.75rem; color: var(--text-muted);">
                        {{ formatFullDateTime(u.created_at) }}
                      </td>
                      <td style="text-align: right;">
                        <div class="user-action-buttons">
                          <button class="action-btn-sm" @click="resetUserPassword(u.id)" title="Reset Password">
                            <i data-lucide="key" style="width: 14px; height: 14px;"></i>
                          </button>
                          <button 
                            v-if="u.username !== 'admin'"
                            class="action-btn-sm" 
                            :class="u.is_blocked ? 'unblock-btn' : 'block-btn'"
                            @click="toggleBlockUser(u.id)" 
                            :title="u.is_blocked ? 'Unblock user' : 'Block user'"
                          >
                            <i :data-lucide="u.is_blocked ? 'unlock' : 'lock'" style="width: 14px; height: 14px;"></i>
                          </button>
                        </div>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>

            <!-- Right Side: Add User Form -->
            <div class="form-panel-side">
              <div class="add-camera-form" style="border: none; padding-top: 0;">
                <h3 class="form-title">Create User Account</h3>
                
                <div class="form-group">
                  <label>Username *</label>
                  <input type="text" placeholder="Enter username" v-model="newUser.username">
                </div>

                <div class="form-group">
                  <label>Password (optional, default 123456)</label>
                  <input type="password" placeholder="Enter password" v-model="newUser.password">
                </div>

                <div class="form-group">
                  <label>Role *</label>
                  <select class="filter-select w-full" v-model="newUser.role">
                    <option value="user">Standard User</option>
                    <option value="admin">Administrator</option>
                  </select>
                </div>

                <div v-if="addUserError" class="form-error">
                  {{ addUserError }}
                </div>

                <button class="submit-btn" :disabled="isAddingUser" @click="addUser">
                  <i data-lucide="user-plus" style="width: 16px; height: 16px;"></i>
                  {{ isAddingUser ? 'Creating...' : 'Create Account' }}
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- ==================== DIAGNOSTICS MODULE ==================== -->
        <div v-else-if="currentModule === 'diagnostics'" class="module-view diagnostics-view">
          <div class="diagnostics-layout">
            <!-- Simulated monospaced terminal -->
            <div class="terminal-panel">
              <div class="terminal-header">
                <div class="terminal-buttons">
                  <span class="btn-dot close"></span>
                  <span class="btn-dot minimize"></span>
                  <span class="btn-dot zoom"></span>
                </div>
                <span class="terminal-title">cameraman_diagnostics_stream.log</span>
                <button 
                  class="clear-terminal-btn" 
                  :disabled="authRole !== 'admin'" 
                  @click="systemLogs = []" 
                  title="Clear console"
                >
                  <i data-lucide="trash-2" style="width: 14px; height: 14px; display: inline-block; vertical-align: middle; margin-right: 4px;"></i>
                  Clear
                </button>
              </div>
              <div class="terminal-body" ref="terminalBody">
                <div v-if="systemLogs.length === 0" class="terminal-line text-muted">
                  Console cleared. Waiting for event logs...
                </div>
                <div 
                  v-for="(log, idx) in systemLogs" 
                  :key="idx" 
                  class="terminal-line"
                  :class="log.type"
                >
                  <span class="log-time">[{{ log.timestamp }}]</span>
                  <span class="log-message">{{ log.message }}</span>
                </div>
              </div>
            </div>

            <!-- Diagnostics Action Toolbar -->
            <div class="diagnostics-sidebar">
              <div class="diagnostics-tool-card">
                <h3>Developer Diagnostics</h3>
                <p style="font-size: 0.8rem; color: var(--text-muted); margin-bottom: 1rem; text-align: left;">
                  Simulate events and webhook interactions with the camera ingestion API.
                </p>

                <div class="tool-action-group">
                  <h4 style="font-size: 0.85rem; margin-bottom: 0.5rem; text-align: left;">CURL Webhook Simulator</h4>
                  <div class="curl-code-snippet">
                    <pre>curl -X POST -H "Content-Type: application/json" \
  -d '{"camera_id": "test-cam", "image": "data:image/jpeg;base64,..."}' \
  http://localhost:8080/api/webhooks/camera</pre>
                  </div>
                  
                  <button 
                    class="submit-btn outline w-full" 
                    :disabled="authRole !== 'admin'" 
                    @click="triggerSimulatedLog" 
                    style="margin-top: 1rem;"
                  >
                    <i data-lucide="terminal" style="width: 16px; height: 16px;"></i>
                    Simulate Webhook event
                  </button>
                  <p v-if="authRole !== 'admin'" style="font-size: 0.7rem; color: var(--status-error); margin-top: 0.5rem; text-align: left;">
                    * Simulation triggers require Administrator privileges.
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- Global Sliding Event Detail Drawer Overlay -->
    <div class="drawer-overlay" :class="{ open: selectedEvent !== null }" @click="selectedEvent = null"></div>
    <div class="drawer-panel" :class="{ open: selectedEvent !== null }">
      <div v-if="selectedEvent" class="drawer-content">
        <!-- Close Button -->
        <button class="drawer-close-btn" @click="selectedEvent = null">
          <i data-lucide="x"></i>
        </button>

        <h2 class="drawer-title">Recognition Analysis</h2>
        
        <div class="drawer-section">
          <!-- Image -->
          <div class="drawer-image-container">
            <img :src="'/uploads/' + selectedEvent.image_path">
            <span class="badge img-overlay-badge" :class="selectedEvent.is_plate_valid ? 'badge-success' : 'badge-error'">
              {{ selectedEvent.is_plate_valid ? 'Valid Plate' : 'Validation Failed' }}
            </span>
          </div>
        </div>

        <div class="drawer-section">
          <!-- License Plate rendered -->
          <div class="plate-box-container">
            <div v-if="selectedEvent.is_plate_valid" class="bangla-plate-render">
              <div class="plate-line-top">
                <span>{{ selectedEvent.district }}</span>
                <span v-if="selectedEvent.metro_prefix">মেট্রো</span>
                <span>{{ selectedEvent.vehicle_class }}</span>
              </div>
              <div class="plate-line-bottom">
                {{ selectedEvent.plate_number }}
              </div>
            </div>
            <div v-else class="plate-invalid-box">
              <i data-lucide="alert-triangle" style="width: 32px; height: 32px; color: var(--status-error); margin-bottom: 0.5rem;"></i>
              <h4>Format Verification Failed</h4>
              <p>The text could not be parsed into a standard Bangladeshi license plate format.</p>
            </div>
          </div>
        </div>

        <div class="drawer-section metadata-section">
          <h3>Surveillance Data</h3>
          <div class="meta-grid">
            <div class="meta-row">
              <span class="meta-label">Camera</span>
              <span class="meta-val">{{ getCameraName(selectedEvent.camera_id) }}</span>
            </div>
            <div class="meta-row" v-if="selectedEvent.camera_id && getCameraLocation(selectedEvent.camera_id)">
              <span class="meta-label">Location</span>
              <span class="meta-val">{{ getCameraLocation(selectedEvent.camera_id) }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">District</span>
              <span class="meta-val">{{ selectedEvent.district || '—' }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Class</span>
              <span class="meta-val">{{ selectedEvent.vehicle_class || '—' }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Plate number</span>
              <span class="meta-val">{{ selectedEvent.plate_number || '—' }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Timestamp</span>
              <span class="meta-val font-mono">{{ formatFullDateTime(selectedEvent.created_at) }}</span>
            </div>
          </div>
        </div>

        <div class="drawer-section raw-response-section">
          <h3>Raw AI Response</h3>
          <div class="raw-response-box">
            {{ selectedEvent.raw_ai_text || 'No raw output' }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
:root {
  --bg-color: #080911;
  --card-bg: rgba(20, 22, 45, 0.6);
  --card-border: rgba(99, 102, 241, 0.15);
  --accent-primary: #6366f1; /* Indigo */
  --accent-glow: rgba(99, 102, 241, 0.35);
  --text-main: #f3f4f6;
  --text-muted: #9ca3af;
  --status-success: #10b981;
  --status-error: #ef4444;
  --status-warning: #f59e0b;
  --plate-bg: #1e293b;
  --plate-border: #f59e0b;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: 'Inter', sans-serif;
  background-color: var(--bg-color);
  color: var(--text-main);
  overflow: hidden;
  min-height: 100vh;
  background-image: 
    radial-gradient(circle at 12% 15%, rgba(99, 102, 241, 0.15) 0%, transparent 45%),
    radial-gradient(circle at 88% 85%, rgba(239, 68, 68, 0.08) 0%, transparent 45%);
}

h1, h2, h3, h4, .brand-text {
  font-family: 'Outfit', sans-serif;
}

/* Page Transitions */
.fade-in {
  animation: fadeIn 0.35s ease-out forwards;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: translateY(0); }
}

/* App Layout Grid */
.app-layout {
  display: flex;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}

/* Login overlay styles */
.login-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  background-color: var(--bg-color);
  background-image: 
    radial-gradient(circle at 50% 50%, rgba(99, 102, 241, 0.2) 0%, transparent 60%),
    radial-gradient(circle at 10% 10%, rgba(239, 68, 68, 0.05) 0%, transparent 35%);
}

.login-card {
  background: rgba(20, 22, 45, 0.65);
  backdrop-filter: blur(25px);
  border: 1px solid var(--card-border);
  width: 100%;
  max-width: 400px;
  padding: 2.5rem;
  border-radius: 20px;
  box-shadow: 0 15px 50px rgba(0, 0, 0, 0.5), 0 0 30px var(--accent-glow);
  text-align: center;
}

.login-brand {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  margin-bottom: 0.5rem;
}

.login-subtitle {
  font-size: 0.85rem;
  color: var(--text-muted);
  margin-bottom: 2rem;
}

.login-form-fields {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  text-align: left;
}

/* Sidebar Navigation */
.sidebar-nav {
  width: 250px;
  background: rgba(13, 15, 33, 0.9);
  border-right: 1px solid var(--card-border);
  backdrop-filter: blur(20px);
  display: flex;
  flex-direction: column;
  padding: 1.5rem 1rem;
  z-index: 10;
}

.nav-brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 2rem;
  padding-left: 0.5rem;
}

.nav-brand .logo-icon {
  background: linear-gradient(135deg, var(--accent-primary), #ec4899);
  width: 32px;
  height: 32px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 0 12px var(--accent-glow);
}

.brand-text {
  font-size: 1.2rem;
  font-weight: 800;
  letter-spacing: 1px;
  background: linear-gradient(to right, #ffffff, #a5b4fc);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.nav-menu {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  flex: 1;
}

.nav-item {
  background: transparent;
  border: none;
  color: var(--text-muted);
  padding: 0.85rem 1rem;
  border-radius: 10px;
  cursor: pointer;
  font-family: inherit;
  font-size: 0.9rem;
  font-weight: 550;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  transition: all 0.3s;
  text-align: left;
}

.nav-item i {
  width: 18px;
  height: 18px;
  opacity: 0.8;
}

.nav-item:hover {
  color: var(--text-main);
  background: rgba(255, 255, 255, 0.03);
}

.nav-item.active {
  color: #ffffff;
  background: var(--accent-primary);
  box-shadow: 0 0 15px var(--accent-glow);
}

.nav-footer {
  border-top: 1px solid var(--card-border);
  padding-top: 1rem;
}

.live-indicator {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.75rem;
  color: var(--text-muted);
  font-weight: 600;
  margin-bottom: 0.75rem;
}

/* User Profile Card in Nav */
.user-profile-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: rgba(17, 19, 39, 0.8);
  border: 1px solid var(--card-border);
  padding: 0.75rem;
  border-radius: 12px;
  margin-top: 0.5rem;
}

.user-profile-info {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  text-align: left;
}

.profile-avatar {
  background: var(--accent-primary);
  width: 30px;
  height: 30px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 0 8px var(--accent-glow);
}

.profile-username {
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--text-main);
  max-width: 110px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.badge-role {
  font-size: 0.65rem;
  font-weight: 700;
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
  text-transform: uppercase;
}

.badge-role.admin { background: rgba(16, 185, 129, 0.2); color: var(--status-success); border: 1px solid rgba(16, 185, 129, 0.3); }
.badge-role.user { background: rgba(99, 102, 241, 0.2); color: #a5b4fc; border: 1px solid rgba(99, 102, 241, 0.3); }

.logout-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 0.25rem;
  border-radius: 4px;
  transition: all 0.3s;
}

.logout-btn:hover {
  color: var(--status-error);
  background: rgba(239, 68, 68, 0.1);
}

/* Pulse effects */
.pulse-green {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--status-success);
  box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7);
  animation: pulseGreenAnim 2s infinite;
}

@keyframes pulseGreenAnim {
  0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }
  70% { transform: scale(1); box-shadow: 0 0 0 6px rgba(16, 185, 129, 0); }
  100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
}

.pulse-amber {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--status-warning);
  box-shadow: 0 0 0 0 rgba(245, 158, 11, 0.7);
  animation: pulseAmberAnim 2s infinite;
}

@keyframes pulseAmberAnim {
  0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(245, 158, 11, 0.7); }
  70% { transform: scale(1); box-shadow: 0 0 0 6px rgba(245, 158, 11, 0); }
  100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(245, 158, 11, 0); }
}

.pulse-gray {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-muted);
  opacity: 0.6;
}

/* Main Content Area */
.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  background: rgba(8, 9, 17, 0.4);
}

.content-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem 2rem;
  border-bottom: 1px solid var(--card-border);
}

.header-text {
  text-align: left;
}

.page-title {
  font-size: 1.6rem;
  font-weight: 800;
  letter-spacing: 0.5px;
}

.page-subtitle {
  font-size: 0.85rem;
  color: var(--text-muted);
  margin-top: 0.2rem;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.polling-status {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  background: rgba(17, 19, 39, 0.6);
  border: 1px solid var(--card-border);
  padding: 0.4rem 0.75rem;
  border-radius: 8px;
  font-size: 0.75rem;
  font-weight: 600;
}

.icon-btn {
  background: rgba(17, 19, 39, 0.6);
  border: 1px solid var(--card-border);
  color: var(--text-main);
  width: 34px;
  height: 34px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.3s;
}

.icon-btn:hover {
  background: var(--accent-primary);
  box-shadow: 0 0 10px var(--accent-glow);
  border-color: var(--accent-primary);
}

.icon-btn i {
  width: 16px;
  height: 16px;
}

/* Workspace Viewport */
.workspace-viewport {
  flex: 1;
  padding: 2rem;
  overflow-y: auto;
}

.workspace-viewport::-webkit-scrollbar {
  width: 6px;
}

.workspace-viewport::-webkit-scrollbar-thumb {
  background: rgba(99, 102, 241, 0.25);
  border-radius: 3px;
}

/* Dashboard Module Styles */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 1.5rem;
  margin-bottom: 2rem;
}

.dashboard-stat-card {
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  border: 1px solid var(--card-border);
  padding: 1.25rem 1.5rem;
  border-radius: 16px;
  display: flex;
  align-items: center;
  gap: 1.25rem;
  box-shadow: 0 8px 32px 0 rgba(0, 0, 0, 0.15);
  text-align: left;
}

.stat-icon-wrapper {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.stat-icon-wrapper i {
  width: 22px;
  height: 22px;
  color: white;
}

.stat-icon-wrapper.blue { background: linear-gradient(135deg, #3b82f6, #1d4ed8); }
.stat-icon-wrapper.green { background: linear-gradient(135deg, #10b981, #047857); }
.stat-icon-wrapper.amber { background: linear-gradient(135deg, #f59e0b, #b45309); }
.stat-icon-wrapper.purple { background: linear-gradient(135deg, #8b5cf6, #5b21b6); }

.stat-label {
  font-size: 0.8rem;
  color: var(--text-muted);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.stat-value {
  font-size: 1.8rem;
  font-weight: 800;
  font-family: 'Outfit', sans-serif;
  margin-top: 0.15rem;
}

.text-green { color: var(--status-success); }
.text-amber { color: var(--status-warning); }
.text-red { color: var(--status-error); }

/* Dashboard detail panes split */
.dashboard-details-grid {
  display: grid;
  grid-template-columns: 1.6fr 1fr;
  gap: 1.5rem;
}

.dashboard-panel {
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  border: 1px solid var(--card-border);
  border-radius: 16px;
  padding: 1.5rem;
  box-shadow: 0 8px 32px 0 rgba(0, 0, 0, 0.15);
  display: flex;
  flex-direction: column;
}

.panel-title {
  font-size: 1.1rem;
  font-weight: 700;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 1.25rem;
  border-bottom: 1px solid var(--card-border);
  padding-bottom: 0.75rem;
  text-align: left;
}

.panel-title-icon {
  width: 18px;
  height: 18px;
  color: var(--accent-primary);
}

.panel-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  gap: 0.75rem;
  min-height: 200px;
}

.panel-empty i {
  width: 36px;
  height: 36px;
}

/* Recent Events List */
.recent-events-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.recent-event-item {
  background: rgba(17, 19, 39, 0.4);
  border: 1px solid var(--card-border);
  border-radius: 12px;
  padding: 0.75rem 1rem;
  display: flex;
  align-items: center;
  gap: 1rem;
  cursor: pointer;
  transition: all 0.3s;
  text-align: left;
}

.recent-event-item:hover {
  transform: translateX(4px);
  background: rgba(99, 102, 241, 0.08);
  border-color: rgba(99, 102, 241, 0.3);
}

.recent-event-thumb {
  width: 50px;
  height: 50px;
  border-radius: 8px;
  overflow: hidden;
  background: #000;
  border: 1px solid var(--card-border);
}

.recent-event-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.recent-event-details {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.recent-event-header {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  color: var(--text-muted);
  font-weight: 500;
}

.event-cam {
  font-weight: 700;
  color: var(--text-main);
}

.recent-event-plate {
  font-size: 0.9rem;
  font-weight: 700;
}

.event-plate-text {
  color: var(--status-success);
}

.event-plate-invalid {
  color: var(--status-error);
  font-size: 0.8rem;
  font-weight: 500;
}

.recent-event-badge {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  font-weight: 600;
}

.badge-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.bg-green { background-color: var(--status-success); }
.bg-red { background-color: var(--status-error); }

/* Breakdown items styles */
.active-ai-card {
  background: rgba(99, 102, 241, 0.05);
  border: 1px solid rgba(99, 102, 241, 0.2);
  border-radius: 12px;
  padding: 1rem;
  margin-bottom: 1.5rem;
  text-align: left;
}

.active-ai-title {
  font-size: 0.9rem;
  font-weight: 700;
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-bottom: 0.75rem;
}

.active-ai-details {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.8rem;
}

.ai-param {
  display: flex;
  justify-content: space-between;
}

.param-lbl {
  color: var(--text-muted);
  font-weight: 500;
}

.param-val {
  color: var(--text-main);
  font-weight: 600;
}

.breakdown-section {
  margin-bottom: 1.5rem;
  text-align: left;
}

.breakdown-title {
  font-size: 0.9rem;
  font-weight: 700;
  margin-bottom: 0.75rem;
  color: var(--text-main);
}

.breakdown-list {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.breakdown-item {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.breakdown-bar-lbl {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--text-muted);
}

.breakdown-bar-bg {
  height: 6px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 3px;
  overflow: hidden;
}

.breakdown-bar-fill {
  height: 100%;
  background: var(--accent-primary);
  border-radius: 3px;
  box-shadow: 0 0 8px var(--accent-glow);
}

.class-badges-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 0.5rem;
}

.class-badge-item {
  background: rgba(17, 19, 39, 0.6);
  border: 1px solid var(--card-border);
  padding: 0.5rem;
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.15rem;
}

.class-letter {
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--accent-primary);
}

.class-count {
  font-size: 0.7rem;
  color: var(--text-muted);
  font-weight: 600;
}

/* Events Feed Module Styles */
.events-toolbar {
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  border: 1px solid var(--card-border);
  border-radius: 16px;
  padding: 1rem 1.5rem;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1.5rem;
  margin-bottom: 1.5rem;
}

.toolbar-search {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  background: rgba(17, 19, 39, 0.6);
  border: 1px solid var(--card-border);
  border-radius: 8px;
  padding: 0.5rem 0.75rem;
}

.toolbar-search i {
  width: 16px;
  height: 16px;
  color: var(--text-muted);
}

.toolbar-search input {
  background: transparent;
  border: none;
  color: var(--text-main);
  outline: none;
  font-family: inherit;
  font-size: 0.85rem;
  width: 100%;
}

.toolbar-filters {
  display: flex;
  gap: 1.25rem;
}

.filter-group-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  text-align: left;
}

.filter-group-item label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.form-select {
  background: rgba(17, 19, 39, 0.8);
  border: 1px solid var(--card-border);
  border-radius: 6px;
  color: var(--text-main);
  padding: 0.4rem 0.75rem;
  font-family: inherit;
  font-size: 0.8rem;
  outline: none;
  cursor: pointer;
}

.btn-group {
  display: flex;
  background: rgba(17, 19, 39, 0.8);
  border: 1px solid var(--card-border);
  border-radius: 6px;
  padding: 0.15rem;
}

.btn-group-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-family: inherit;
  font-size: 0.75rem;
  font-weight: 600;
  padding: 0.3rem 0.75rem;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.3s;
}

.btn-group-btn.active {
  color: white;
  background: var(--accent-primary);
}

.events-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  min-height: 350px;
  color: var(--text-muted);
}

.events-grid-container {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 1.5rem;
}

.event-grid-card {
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  border: 1px solid var(--card-border);
  border-radius: 14px;
  overflow: hidden;
  cursor: pointer;
  transition: all 0.3s;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
}

.event-grid-card:hover {
  transform: translateY(-4px);
  box-shadow: 0 8px 32px var(--accent-glow);
}

.event-card-img {
  height: 140px;
  position: relative;
  background: #000;
}

.event-card-img img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.event-card-time {
  position: absolute;
  bottom: 0.5rem;
  right: 0.5rem;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(4px);
  font-size: 0.7rem;
  font-weight: 600;
  padding: 0.2rem 0.5rem;
  border-radius: 4px;
}

.event-card-content {
  padding: 1rem;
  text-align: left;
}

.event-card-meta {
  display: flex;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}

.cam-lbl {
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--text-muted);
}

.event-card-plate {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 60px;
}

.card-plate-rendered {
  background: var(--plate-bg);
  border: 2px solid var(--plate-border);
  border-radius: 6px;
  width: 100%;
  padding: 0.25rem;
  text-align: center;
  color: white;
  box-shadow: inset 0 0 5px rgba(0,0,0,0.5);
}

.card-plate-top {
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.card-plate-bottom {
  font-size: 0.95rem;
  font-weight: 700;
}

.card-plate-error {
  color: var(--status-error);
  font-size: 0.8rem;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
}

.border-green-glow:hover {
  border-color: var(--status-success);
}

.border-red-glow:hover {
  border-color: var(--status-error);
}

/* Camera Registry Screen Styles */
.cameras-layout-grid {
  display: grid;
  grid-template-columns: 1.6fr 1fr;
  gap: 1.5rem;
  align-items: start;
}

.cameras-panel {
  background: var(--card-bg);
  border: 1px solid var(--card-border);
  border-radius: 16px;
  padding: 1.5rem;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.15);
}

.cameras-cards-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 1.25rem;
}

.camera-detail-card {
  background: rgba(17, 19, 39, 0.5);
  border: 1px solid var(--card-border);
  border-radius: 14px;
  overflow: hidden;
  padding: 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.camera-stream-box {
  height: 120px;
  background: #0d0f23;
  border-radius: 8px;
  position: relative;
  overflow: hidden;
}

.camera-scanline {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 2px;
  background: rgba(99, 102, 241, 0.4);
  box-shadow: 0 0 8px var(--accent-primary);
  animation: scanAnim 3s linear infinite;
  z-index: 2;
}

@keyframes scanAnim {
  0% { top: 0; }
  100% { top: 120px; }
}

.camera-overlay-info {
  position: absolute;
  top: 0.5rem;
  left: 0.5rem;
  display: flex;
  align-items: center;
  gap: 0.35rem;
  background: rgba(0,0,0,0.6);
  padding: 0.2rem 0.5rem;
  border-radius: 4px;
  z-index: 3;
}

.camera-feed-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.red-pulse {
  background: var(--status-error);
  box-shadow: 0 0 6px var(--status-error);
  animation: redPulseAnim 1.5s infinite;
}

@keyframes redPulseAnim {
  0% { opacity: 0.5; }
  50% { opacity: 1; }
  100% { opacity: 0.5; }
}

.feed-title {
  font-size: 0.65rem;
  font-weight: 700;
  color: white;
}

.camera-stream-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: rgba(255, 255, 255, 0.1);
  position: relative;
}

.stream-icon-bg {
  width: 48px;
  height: 48px;
}

.camera-stream-data {
  position: absolute;
  bottom: 0.5rem;
  left: 0.5rem;
  color: rgba(255, 255, 255, 0.45);
  font-family: monospace;
  font-size: 0.65rem;
}

.camera-card-text {
  text-align: left;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.camera-name-text {
  font-weight: 750;
  font-size: 1rem;
}

.camera-loc-val {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.camera-actions-row {
  margin-top: auto;
}

.delete-btn-card {
  width: 100%;
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.2);
  border-radius: 6px;
  color: var(--status-error);
  padding: 0.5rem;
  font-family: inherit;
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s;
}

.delete-btn-card:hover {
  background: var(--status-error);
  color: white;
}

.form-panel-side {
  background: var(--card-bg);
  border: 1px solid var(--card-border);
  border-radius: 16px;
  padding: 1.5rem;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.15);
}

.read-only-banner {
  border: 1px dashed var(--card-border);
  padding: 1.5rem;
  border-radius: 12px;
  background: rgba(99, 102, 241, 0.02);
}

/* AI Config Styles */
.ai-configs-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 1.25rem;
}

.ai-config-detail-card {
  background: rgba(17, 19, 39, 0.5);
  border: 1px solid var(--card-border);
  border-radius: 14px;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  transition: all 0.3s;
}

.ai-config-detail-card.active-glowing {
  border-color: var(--status-success);
  box-shadow: 0 0 15px rgba(16, 185, 129, 0.2);
  background: rgba(16, 185, 129, 0.03);
}

.ai-card-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  text-align: left;
}

.ai-card-meta {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.ai-card-name {
  font-weight: 750;
  font-size: 1rem;
}

.ai-provider-badge {
  font-size: 0.65rem;
  font-weight: 700;
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  text-transform: uppercase;
  display: inline-block;
  width: fit-content;
}

.ai-provider-badge.gemini { background: #3b82f6; color: white; }
.ai-provider-badge.minimax { background: #ec4899; color: white; }
.ai-provider-badge.mock { background: #6b7280; color: white; }

.active-pill {
  font-size: 0.7rem;
  font-weight: 700;
  color: var(--status-success);
  display: flex;
  align-items: center;
  gap: 0.35rem;
}

.ai-card-body-details {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  font-size: 0.8rem;
  text-align: left;
  border-top: 1px dashed var(--card-border);
  padding-top: 0.5rem;
}

.ai-detail-row {
  display: flex;
  justify-content: space-between;
}

.ai-lbl {
  color: var(--text-muted);
}

.ai-val {
  color: var(--text-main);
  font-weight: 600;
}

.ai-card-actions {
  display: flex;
  gap: 0.5rem;
  margin-top: auto;
}

.activate-btn {
  flex: 1;
  background: var(--status-success);
  border: none;
  color: white;
  border-radius: 6px;
  padding: 0.4rem;
  font-family: inherit;
  font-size: 0.75rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s;
}

.activate-btn:hover {
  opacity: 0.9;
}

.delete-ai-btn {
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.2);
  color: var(--status-error);
  border-radius: 6px;
  padding: 0.4rem 0.75rem;
  font-family: inherit;
  font-size: 0.75rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.3s;
}

.delete-ai-btn:hover {
  background: var(--status-error);
  color: white;
}

/* Users Table Styles */
.users-list-table-container {
  overflow-x: auto;
}

.users-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.users-table th {
  padding: 0.75rem 1rem;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-muted);
  border-bottom: 1px solid var(--card-border);
}

.users-table td {
  padding: 1rem;
  font-size: 0.85rem;
  border-bottom: 1px solid var(--card-border);
}

.users-table tr:hover {
  background: rgba(255, 255, 255, 0.02);
}

.user-row-blocked {
  opacity: 0.6;
}

.status-indicator-text {
  font-size: 0.75rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
}

.status-indicator-text::before {
  content: '';
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
}

.status-indicator-text.active { color: var(--status-success); }
.status-indicator-text.active::before { background: var(--status-success); }
.status-indicator-text.blocked { color: var(--status-error); }
.status-indicator-text.blocked::before { background: var(--status-error); }

.user-action-buttons {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
}

.action-btn-sm {
  background: rgba(17, 19, 39, 0.6);
  border: 1px solid var(--card-border);
  color: var(--text-muted);
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.3s;
}

.action-btn-sm:hover {
  color: var(--text-main);
  border-color: var(--accent-primary);
  background: rgba(99, 102, 241, 0.1);
}

.action-btn-sm.block-btn:hover {
  color: var(--status-error);
  border-color: var(--status-error);
  background: rgba(239, 68, 68, 0.1);
}

.action-btn-sm.unblock-btn {
  color: var(--status-error);
  border-color: var(--status-error);
  background: rgba(239, 68, 68, 0.05);
}

.action-btn-sm.unblock-btn:hover {
  color: var(--status-success);
  border-color: var(--status-success);
  background: rgba(16, 185, 129, 0.1);
}

/* Diagnostics & Live logs styles */
.diagnostics-layout {
  display: grid;
  grid-template-columns: 1.6fr 1fr;
  gap: 1.5rem;
  height: calc(100vh - 150px);
}

.terminal-panel {
  background: #07080f;
  border: 1px solid #1f2937;
  border-radius: 12px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: 0 10px 40px rgba(0,0,0,0.5);
}

.terminal-header {
  background: #0f111a;
  border-bottom: 1px solid #1f2937;
  padding: 0.6rem 1rem;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.terminal-buttons {
  display: flex;
  gap: 0.4rem;
}

.btn-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.btn-dot.close { background: #ef4444; }
.btn-dot.minimize { background: #f59e0b; }
.btn-dot.zoom { background: #10b981; }

.terminal-title {
  color: #9ca3af;
  font-family: monospace;
  font-size: 0.75rem;
}

.clear-terminal-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-family: inherit;
  font-size: 0.75rem;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 0.2rem;
  transition: color 0.3s;
}

.clear-terminal-btn:hover:not(:disabled) {
  color: var(--status-error);
}

.clear-terminal-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.terminal-body {
  flex: 1;
  padding: 1.25rem;
  overflow-y: auto;
  font-family: 'Fira Code', 'Courier New', Courier, monospace;
  font-size: 0.8rem;
  text-align: left;
  display: flex;
  flex-direction: column-reverse; /* Shows latest logs on top */
  gap: 0.5rem;
}

.terminal-body::-webkit-scrollbar {
  width: 6px;
}

.terminal-body::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 3px;
}

.terminal-line {
  line-height: 1.4;
  word-break: break-all;
}

.terminal-line.info { color: #22d3ee; }
.terminal-line.success { color: #34d399; }
.terminal-line.warning { color: #facc15; }

.log-time {
  color: #6b7280;
  margin-right: 0.5rem;
}

.diagnostics-sidebar {
  display: flex;
  flex-direction: column;
}

.diagnostics-tool-card {
  background: var(--card-bg);
  border: 1px solid var(--card-border);
  padding: 1.5rem;
  border-radius: 16px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.15);
}

.diagnostics-tool-card h3 {
  font-size: 1.1rem;
  font-weight: 700;
  margin-bottom: 0.5rem;
  text-align: left;
}

.curl-code-snippet {
  background: #07080f;
  border: 1px solid #1f2937;
  border-radius: 8px;
  padding: 0.75rem;
  font-family: monospace;
  font-size: 0.7rem;
  color: #34d399;
  text-align: left;
  overflow-x: auto;
}

.curl-code-snippet pre {
  white-space: pre-wrap;
  word-break: break-all;
}

/* Event Detail Sliding Drawer */
.drawer-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  z-index: 20;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.35s ease;
}

.drawer-overlay.open {
  opacity: 1;
  pointer-events: auto;
}

.drawer-panel {
  position: fixed;
  top: 0;
  right: 0;
  width: 440px;
  height: 100vh;
  background: rgba(14, 16, 38, 0.98);
  border-left: 1px solid var(--card-border);
  box-shadow: -10px 0 40px rgba(0,0,0,0.5);
  backdrop-filter: blur(25px);
  z-index: 25;
  transform: translateX(100%);
  transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1);
  overflow-y: auto;
}

.drawer-panel.open {
  transform: translateX(0);
}

.drawer-content {
  padding: 2rem;
  position: relative;
  text-align: left;
}

.drawer-close-btn {
  position: absolute;
  top: 1.5rem;
  right: 1.5rem;
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  transition: color 0.3s;
}

.drawer-close-btn:hover {
  color: var(--status-error);
}

.drawer-close-btn i {
  width: 20px;
  height: 20px;
}

.drawer-title {
  font-size: 1.4rem;
  font-weight: 800;
  margin-bottom: 1.5rem;
  color: white;
  border-bottom: 1px solid var(--card-border);
  padding-bottom: 0.75rem;
}

.drawer-section {
  margin-bottom: 1.5rem;
}

.drawer-image-container {
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid var(--card-border);
  position: relative;
}

.drawer-image-container img {
  width: 100%;
  display: block;
}

.img-overlay-badge {
  position: absolute;
  top: 0.75rem;
  left: 0.75rem;
  backdrop-filter: blur(4px);
  font-size: 0.75rem;
  font-weight: 700;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
}

.badge-success { background: rgba(16, 185, 129, 0.85); color: white; }
.badge-error { background: rgba(239, 68, 68, 0.85); color: white; }

.plate-box-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  background: rgba(17, 19, 39, 0.5);
  border: 1px solid var(--card-border);
  border-radius: 12px;
  padding: 1.25rem;
}

.bangla-plate-render {
  background: var(--plate-bg);
  border: 3px solid var(--plate-border);
  border-radius: 8px;
  padding: 0.5rem 1rem;
  width: 190px;
  text-align: center;
  color: white;
  font-weight: 700;
  margin-bottom: 1rem;
  box-shadow: inset 0 0 10px rgba(0,0,0,0.8), 0 0 15px rgba(245, 158, 11, 0.15);
  position: relative;
}

/* Mounting bolts */
.bangla-plate-render::before, .bangla-plate-render::after {
  content: '';
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 8px;
  height: 8px;
  background: #64748b;
  border-radius: 50%;
  box-shadow: 1px 1px 2px rgba(0,0,0,0.5);
}

.bangla-plate-render::before { left: 8px; }
.bangla-plate-render::after { right: 8px; }

.plate-line-top {
  font-size: 1rem;
  letter-spacing: 1px;
  display: flex;
  justify-content: center;
  gap: 0.5rem;
  border-bottom: 1px solid rgba(255,255,255,0.15);
  padding-bottom: 0.2rem;
  margin-bottom: 0.2rem;
}

.plate-line-bottom {
  font-size: 1.45rem;
}

.plate-invalid-box {
  text-align: center;
  color: var(--status-error);
}

.plate-invalid-box h4 {
  font-size: 0.95rem;
  font-weight: 700;
  margin-top: 0.25rem;
}

.plate-invalid-box p {
  font-size: 0.75rem;
  color: var(--text-muted);
  margin-top: 0.2rem;
}

.metadata-section h3, .raw-response-section h3 {
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--text-main);
  margin-bottom: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.meta-grid {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  background: rgba(17, 19, 39, 0.4);
  border: 1px solid var(--card-border);
  border-radius: 10px;
  padding: 0.85rem 1rem;
}

.meta-row {
  display: flex;
  justify-content: space-between;
  font-size: 0.8rem;
}

.meta-label {
  color: var(--text-muted);
  font-weight: 500;
}

.meta-val {
  color: var(--text-main);
  font-weight: 600;
}

.raw-response-box {
  background: rgba(17, 19, 39, 0.6);
  border: 1px solid var(--card-border);
  border-radius: 10px;
  padding: 1rem;
  font-family: monospace;
  font-size: 0.75rem;
  line-height: 1.4;
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 160px;
  overflow-y: auto;
}

/* Forms General */
.add-camera-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  text-align: left;
}

.form-title {
  font-size: 1.1rem;
  font-weight: 800;
  margin-bottom: 0.5rem;
  color: white;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.form-group label {
  font-size: 0.75rem;
  color: var(--text-muted);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.form-group input, .form-group select, .filter-select {
  background: rgba(17, 19, 39, 0.7);
  border: 1px solid var(--card-border);
  border-radius: 8px;
  padding: 0.6rem 0.85rem;
  color: var(--text-main);
  font-family: inherit;
  font-size: 0.85rem;
  outline: none;
  transition: all 0.3s;
}

.form-group input:focus, .form-group select:focus {
  border-color: var(--accent-primary);
  box-shadow: 0 0 10px var(--accent-glow);
}

.form-error {
  color: var(--status-error);
  font-size: 0.75rem;
  font-weight: 600;
}

.submit-btn {
  background: var(--accent-primary);
  border: none;
  border-radius: 8px;
  color: white;
  padding: 0.7rem;
  font-family: inherit;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  transition: all 0.3s;
  box-shadow: 0 4px 12px var(--accent-glow);
}

.submit-btn:hover:not(:disabled) {
  opacity: 0.9;
  box-shadow: 0 4px 18px var(--accent-glow);
}

.submit-btn.outline {
  background: transparent;
  border: 1px solid var(--accent-primary);
  color: var(--accent-primary);
  box-shadow: none;
}

.submit-btn.outline:hover {
  background: var(--accent-primary);
  color: white;
}

.submit-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.w-full { width: 100%; }
</style>
