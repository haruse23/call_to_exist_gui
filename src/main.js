import { createApp } from 'vue'
import App from './App.vue'
import PrimeVue from 'primevue/config'
import Aura from '@primevue/themes/aura'
import 'primeicons/primeicons.css'
import { definePreset } from '@primevue/themes'

const app = createApp(App);

app.use(PrimeVue, {
	license: 'eyJpZCI6IjliMWI3NjEzLTA3M2EtNGY2ZC04YWVjLTI1ZTA4MDg1NDI0NCIsInByb2R1Y3QiOiJwcmltZXVpIiwidGllciI6ImNvbW11bml0eSIsInR5cGUiOiJkZXYiLCJpYXQiOjE3ODgwMzMwMDAsImV4cCI6MTgxOTU2OTAwMH0.K1t1-3nc9PcuDqttLwMwttLXhwg3tKPE1MEiZYg_ZLh3fersgLTzz46ryRAeI35WqmbMI4yllsaNa9X09d_RDw',
	
	theme: {
		preset: Aura,
		
		options: {
            darkModeSelector: '.my-app-dark',
        }
		
		
	}
	
});
app.mount('#app');