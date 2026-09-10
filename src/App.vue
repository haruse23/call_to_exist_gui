<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import Tree from 'primevue/tree'
import Button from 'primevue/button'
import Tabs from 'primevue/tabs'
import TabList from 'primevue/tablist'
import Tab from 'primevue/tab'
import TabPanels from 'primevue/tabpanels'
import TabPanel from 'primevue/tabpanel'
import Checkbox from 'primevue/checkbox'
import InputText from 'primevue/inputtext'

import DataTable from 'primevue/datatable'
import Column from 'primevue/column'
import Dialog from 'primevue/dialog'

import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { save } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { readFile } from '@tauri-apps/plugin-fs'
import { writeFile } from '@tauri-apps/plugin-fs'
import { listen } from '@tauri-apps/api/event'

import * as THREE from "three"
import { OrbitControls } from "three/addons/controls/OrbitControls.js"
import { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js'

import './assets/main.css'

// Tab active state (matches tab values)
const activeTab = ref('0')

// Tree state for Unpack
const nodes = ref([])
const nodesPAC = ref([])

// DataTable
const datatable_rows = ref([])
const datatable_rowsPAC = ref([])

// Stores selected keys in PrimeVue tree format: { 'key1': { checked: true, partialChecked: false } }
const selectedKeys = ref({})
const selectedKeysPAC = ref({})


// Stores expanded keys
const expandedKeys = ref({})
const expandedKeysPAC = ref({})


// Filter through search
const searchText = ref('')       // Tree
const searchTextPAC = ref('')


const PacDecompressChecked = ref(true)

// Node Counter
function countNodes(nodes) {
    let folders = 0
    let files = 0

    for (const node of nodes) {
        if (node.node_type === "Folder") {
            folders++

            if (node.children?.length) {
                const result = countNodes(node.children)
                folders += result.folders
                files += result.files
            }
        } else {
            files++
        }
    }

    return { folders, files }
}


// Dialog

// CPK extraction progress
const filecountCPK = ref(0)
const finished_countCPK = ref(0)
const visibleCPK = ref(false)

// PAC extraction progress
const filecountPAC = ref(0)
const finished_countPAC = ref(0)
const visiblePAC = ref(false)

onMounted(async () => {

    // CPK
    await listen('extraction-progress', (event) => {
        finished_countCPK.value = event.payload[0]
        filecountCPK.value = event.payload[1]

        console.log(
            'CPK progress:',
            finished_countCPK.value,
            '/',
            filecountCPK.value
        )
    })

    // PAC
    await listen('pac-extraction-progress', (event) => {
        finished_countPAC.value = event.payload[0]
        filecountPAC.value = event.payload[1]

        console.log(
            'PAC progress:',
            finished_countPAC.value,
            '/',
            filecountPAC.value
        )
    })
})

// Stop Extraction Button
async function stopExtraction() {
    try {
        await invoke('stop_extraction')
    } catch (error) {
        console.error('Failed to stop extraction:', error)
    }
}


// Tree Info
const totalsCPK = computed(() => countNodes(nodes.value))

const selectedtotalsCPK = computed(() => {
    const selectedTree = getSelectedNodes(
        nodes.value,
        selectedKeys.value
    )

    return countNodes(selectedTree)
})

const totalsPAC = computed(() => countNodes(nodesPAC.value))

const selectedtotalsPAC = computed(() => {
    const selectedTree = getSelectedNodes(
        nodesPAC.value,
        selectedKeysPAC.value
    )

    return countNodes(selectedTree)
})


const filteredTableRows = computed(() => {
    if (!searchText.value) {
        return datatable_rows.value
    }

    const search = searchText.value.toLowerCase()

    return datatable_rows.value.filter(row =>
        String(row.dir_name ?? '').toLowerCase().includes(search) ||
        String(row.file_name ?? '').toLowerCase().includes(search) ||
        String(row.file_offset ?? '').includes(search) ||
        formatBytes(row.file_size).toLowerCase().includes(search) ||
        formatBytes(row.extract_size).toLowerCase().includes(search)
    )
})

const filteredTableRowsPAC = computed(() => {
    if (!searchTextPAC.value) {
        return datatable_rowsPAC.value
    }

    const search = searchTextPAC.value.toLowerCase()

    return datatable_rowsPAC.value.filter(row =>
        String(row.dir_name ?? '').toLowerCase().includes(search) ||
        String(row.file_name ?? '').toLowerCase().includes(search) ||
        String(row.file_offset ?? '').includes(search) ||
        formatBytes(row.file_size).toLowerCase().includes(search) ||
        formatBytes(row.extract_size).toLowerCase().includes(search)
    )
})

// Computed property to check if all nodes are currently selected
const isAllSelected = computed(() => {
    if (nodes.value.length === 0) return false

    return (
        Object.keys(selectedKeys.value).length ===
        getAllKeys(nodes.value).length
    )
})

const isAllSelectedPAC = computed(() => {
    if (nodesPAC.value.length === 0) return false

    return (
        Object.keys(selectedKeysPAC.value).length ===
        getAllKeys(nodesPAC.value).length
    )
})

// Recursively extract all keys from tree nodes
function getAllKeys(nodeList) {
  let keys = []
  for (const node of nodeList) {
    keys.push(node.key)
    if (node.children && node.children.length > 0) {
      keys = keys.concat(getAllKeys(node.children))
    }
  }
  return keys
}

// Select All / Deselect All Handler
function toggleSelectAll() {
    if (isAllSelected.value) {
        selectedKeys.value = {}
    } else {
        const allKeys = getAllKeys(filteredNodes.value)
        const newSelection = {}

        allKeys.forEach(key => {
            newSelection[key] = {
                checked: true,
                partialChecked: false
            }
        })

        selectedKeys.value = newSelection
    }
}

function toggleSelectAllPAC() {
    if (isAllSelectedPAC.value) {
        selectedKeysPAC.value = {}
    } else {
        const allKeys = getAllKeys(filteredNodesPAC.value)
        const newSelection = {}

        allKeys.forEach(key => {
            newSelection[key] = {
                checked: true,
                partialChecked: false
            }
        })

        selectedKeysPAC.value = newSelection
    }
}



// Helper function to recursively find selected node objects
function getSelectedNodes(nodeList, selectedMap) {
    const result = []

    for (const node of nodeList) {
        const state = selectedMap[node.key]

        const isSelected =
            state && (state.checked || state.partialChecked)

        if (!isSelected) {
            continue
        }

        const selectedNode = {
            key: node.key,
            label: node.label,
            node_type: node.node_type,
            children: [],
            path: node.path,
			contained_type: node.contained_type
        }

        if (node.children && node.children.length > 0) {
            selectedNode.children = getSelectedNodes(
                node.children,
                selectedMap
            )
        }

        result.push(selectedNode)
    }

    return result
}


// Open CPK Archive Function
async function openArchive() {
  const filePaths = await open({
    multiple: true,
    directory: false
  })

  if (filePaths) {
    const result = await invoke('open_cpk_command', {
      paths: filePaths,
      nodes: nodes.value,
      rows: datatable_rows.value
    })
	
	console.log("RUST RESULT:", result)
	console.log("KEYS:", Object.keys(result))
	
    nodes.value = result.tree
    datatable_rows.value = result.rows
  }
}



async function extractSelectedFiles() {
    console.log("1: function called")

    const folderPath = await open({
        multiple: false,
        directory: true
    })

    console.log("2: folder:", folderPath)

    if (folderPath) {
        const selectedTree = getSelectedNodes(
            nodes.value,
            selectedKeys.value
        )

        console.log("3: selected:", selectedTree)

        try {
            console.log("4: invoking Rust")
				
			visibleCPK.value = true
			filecountCPK.value = countNodes(selectedTree).files
			finished_countCPK.value = 0
		
            const result = await invoke('extract_cpk_command', {
                path: folderPath,
                nodes: selectedTree,
				checked: PacDecompressChecked.value
            })
			
			visibleCPK.value = false
			

            console.log("5: Rust returned:", result)
        } catch (error) {
            console.error("RUST ERROR:", error)
        }
    }
}


async function extractSelectedFilesPAC() {
    console.log("1: function called")

    const folderPath = await open({
        multiple: false,
        directory: true
    })

    console.log("2: folder:", folderPath)

    if (folderPath) {
        const selectedTree = getSelectedNodes(
            nodesPAC.value,
            selectedKeysPAC.value
        )

        console.log("3: selected:", selectedTree)

        try {
            console.log("4: invoking Rust")
			
			visiblePAC.value = true
			filecountPAC.value = countNodes(selectedTree).files	
			finished_countPAC.value = 0
			
            const result = await invoke('extract_pac_command', {
                path: folderPath,
                nodes: selectedTree
            })
			
			visiblePAC.value = false
			
			
            console.log("5: Rust returned:", result)
        } catch (error) {
            console.error("RUST ERROR:", error)
        }
    }
}


// FilteredNodes through Search Input

const filteredNodes = computed(() => {
    if (!searchText.value)
        return nodes.value

    function filter(tree) {
        return tree
            .map(node => {
                const children = node.children
                    ? filter(node.children)
                    : []

                const matches =
                    node.label.toLowerCase()
                        .includes(searchText.value.toLowerCase())

                if (matches || children.length > 0) {
                    return {
                        ...node,
                        children
                    }
                }

                return null
            })
            .filter(node => node !== null)
    }

    return filter(nodes.value)
})

const filteredNodesPAC = computed(() => {
    if (!searchTextPAC.value)
        return nodesPAC.value

    function filter(tree) {
        return tree
            .map(node => {
                const children = node.children
                    ? filter(node.children)
                    : []

                const matches =
                    node.label
                        .toLowerCase()
                        .includes(searchTextPAC.value.toLowerCase())

                if (matches || children.length > 0) {
                    return {
                        ...node,
                        children
                    }
                }

                return null
            })
            .filter(node => node !== null)
    }

    return filter(nodesPAC.value)
})


// Open PAC Archive Function
async function openPACArchive() {
  const filePaths = await open({
    multiple: true,
    directory: false
  })

  if (filePaths) {
    const result = await invoke('open_pac_command', { // tauri command function
      paths: filePaths,
      nodes: nodesPAC.value,
	  rows: datatable_rowsPAC.value
    })
	
	nodesPAC.value = result.tree
	datatable_rowsPAC.value = result.rows
	
  }
}



function formatBytes(bytes) {
    if (bytes === 0) return '0 B'

    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    const i = Math.floor(Math.log(bytes) / Math.log(1024))

    return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${units[i]}`
}


const imageSrc = ref(null)

async function LoadTexture(filePath) {
  const pngBytes = await invoke('view_texture_command', {
    path: filePath
  })

  const blob = new Blob(
    [new Uint8Array(pngBytes)],
    { type: 'image/png' }
  )

  imageSrc.value = URL.createObjectURL(blob)
}

async function ViewTexture(filePath = null) {

    // If no path was supplied, open the file picker
    if (!filePath) {
        filePath = await open({
            multiple: false,
            directory: false
        })
    }

    if (filePath) {
        await LoadTexture(filePath)
    }
}



async function BatchConvertPNG() {
  const filePaths = await open({
    multiple: true,
    directory: false
  })

  if (filePaths) {
    const pngBytes = await invoke('batch_convert_texture_command', { // tauri command function
      paths: filePaths,

    })
	
	
	}
	
}


let scene;
let camera;
let renderer;
let modelGroup;

let controls;
let animationId = null;
let animationRunning = null;

const textureLoader = new THREE.TextureLoader()

// -------------------------
// Three.js initialization
// -------------------------

function initViewer() {
    const container = document.getElementById("model-viewer");

    console.log("CONTAINER:", container);
    console.log("SIZE:", container.clientWidth, container.clientHeight);

    scene = new THREE.Scene();

    camera = new THREE.PerspectiveCamera(
        75,
        container.clientWidth / container.clientHeight,
        0.1,
        10000
    );

    camera.position.set(0, 0, 5);

    renderer = new THREE.WebGLRenderer({
        antialias: true
    });

    renderer.setSize(
        container.clientWidth,
        container.clientHeight
    );

    container.appendChild(renderer.domElement);

    modelGroup = new THREE.Group();
    scene.add(modelGroup);

    controls = new OrbitControls(
        camera,
        renderer.domElement
    );

    controls.enableDamping = true;

    const light = new THREE.DirectionalLight(
        0xffffff,
        3
    );

    light.position.set(5, 5, 5);
    scene.add(light);

    scene.add(
        new THREE.AmbientLight(0xffffff, 1)
    );

    animationRunning = true
    animate()
}


watch(activeTab, async (newTab) => {
    if (newTab === '3' && !renderer) {
        await new Promise(resolve => setTimeout(resolve, 50))

        initViewer()
    }
})

// -------------------------
// Load model from Rust
// -------------------------

async function selectModel() {
  const filePath = await open({
    multiple: false,
    directory: false
  })

  if (filePath) {
    await LoadModel(filePath)
  }
}

async function LoadModel(filePath) {
  const meshes = await invoke("view_model_command", {
    path: filePath
  })

  displayMeshes(meshes)

  frameModel()
}


// -------------------------
// Applying textures to meshes
// -------------------------

async function ApplyTexture(filePath, mesh) {

    if (!mesh) {
        console.log("No mesh selected")
        return
    }

    const lowerPath = filePath.toLowerCase()

    // Normal PNG
    if (lowerPath.endsWith(".png")) {

        const pngBytes = await readFile(filePath)

        const blob = new Blob(
            [pngBytes],
            { type: "image/png" }
        )

        const url = URL.createObjectURL(blob)

        textureLoader.load(url, (texture) => {

            texture.colorSpace = THREE.SRGBColorSpace

            mesh.material = new THREE.MeshBasicMaterial({
                map: texture,
                side: THREE.DoubleSide
            })

            URL.revokeObjectURL(url)
        })

        return
    }

    // Game texture, including extensionless files like c001_tex_a_re_tex
    const pngBytes = await invoke("view_texture_command", {
        path: filePath
    })

    const blob = new Blob(
        [new Uint8Array(pngBytes)],
        { type: "image/png" }
    )

    const url = URL.createObjectURL(blob)

    textureLoader.load(url, (texture) => {

        texture.colorSpace = THREE.SRGBColorSpace

        mesh.material = new THREE.MeshBasicMaterial({
            map: texture,
            side: THREE.DoubleSide
        })

        URL.revokeObjectURL(url)
    })
}

// -------------------------
// Get the mesh under the texture drop/mouse cursor 
// -------------------------

let mouse = new THREE.Vector2()
let raycaster = new THREE.Raycaster()


function updateMousePosition(x, y) {
    const rect = renderer.domElement.getBoundingClientRect()

    mouse.x = ((x - rect.left) / rect.width) * 2 - 1
    mouse.y = -((y - rect.top) / rect.height) * 2 + 1
}


function getMeshAtPosition(x, y) {
    updateMousePosition(x, y)

    raycaster.setFromCamera(mouse, camera)

    const meshes = []

    modelGroup.traverse((object) => {
        if (object.isMesh) {
            meshes.push(object)
        }
    })

    const intersections = raycaster.intersectObjects(meshes, false)

    if (intersections.length > 0) {
        return intersections[0].object
    }

    return null
}


// -------------------------
// Convert Rust meshes
// → Three.js meshes
// -------------------------

function displayMeshes(meshes) {
  modelGroup.traverse((object) => {
		if (object.isMesh) {
			object.geometry.dispose()

			if (object.material) {
				object.material.dispose()
			}
		}
	})

  modelGroup.clear()
	
  for (const mesh of meshes) {
	for (const lod of mesh) {
		const geometry = new THREE.BufferGeometry()

		const positions = []
		const normals = []
		const uvs = []

		for (const vertex of lod.vertices) {
		  positions.push(
			vertex.positions[0],
			vertex.positions[1],
			vertex.positions[2]
		  )

		  normals.push(
			vertex.normals[0],
			vertex.normals[1],
			vertex.normals[2]
		  )

		  uvs.push(
			vertex.texcoords[0],
			vertex.texcoords[1]
		  )
		}

		geometry.setAttribute(
		  "position",
		  new THREE.Float32BufferAttribute(positions, 3)
		)

		geometry.setAttribute(
		  "normal",
		  new THREE.Float32BufferAttribute(normals, 3)
		)

		geometry.setAttribute(
		  "uv",
		  new THREE.Float32BufferAttribute(uvs, 2)
		)

		const indices = []

		for (let i = 0; i < lod.indices.length; i += 3) {
			indices.push(
				lod.indices[i].index,
				lod.indices[i + 2].index,
				lod.indices[i + 1].index
			)
		}

		geometry.setIndex(indices)

		const material = new THREE.MeshStandardMaterial({
			color: 0xffffff,
			roughness: 0.8,
			metalness: 0.0,
			side: THREE.FrontSide
		})
		
		const skinIndices = []
		const skinWeights = []

		for (const vertex of lod.vertices) {

			skinIndices.push(
				vertex.blendindices[0],
				vertex.blendindices[1],
				vertex.blendindices[2],
				vertex.blendindices[3]
			)

			skinWeights.push(
				vertex.blendweights[0],
				vertex.blendweights[1],
				vertex.blendweights[2],
				vertex.blendweights[3]
			)
		}

		geometry.setAttribute(
			"skinIndex",
			new THREE.Uint16BufferAttribute(skinIndices, 4)
		)

		geometry.setAttribute(
			"skinWeight",
			new THREE.Float32BufferAttribute(skinWeights, 4)
		)


		const threeMesh = new THREE.Mesh(
		  geometry,
		  material
		)

		modelGroup.add(threeMesh)
	  }
    }
}



function frameModel() {
  const box = new THREE.Box3().setFromObject(modelGroup)

  const center = box.getCenter(new THREE.Vector3())
  const size = box.getSize(new THREE.Vector3())

  const maxDim = Math.max(size.x, size.y, size.z)

  camera.position.set(
    center.x,
    center.y,
    center.z + maxDim * 2
  )

  controls.target.copy(center)
  controls.update()
}


// -------------------------
// Render loop
// -------------------------

function animate() {
    if (!animationRunning || !renderer) {
        return
    }

    animationId = requestAnimationFrame(animate)

    controls?.update()

    renderer.render(scene, camera)
}

// -------------------------
// Export to GLTF
// -------------------------

async function ExportModel() {

    if (!modelGroup) {
        console.log("No model loaded")
        return
    }

    try {

        // Create exporter
        const exporter = new GLTFExporter()

        // Convert Three.js model to GLB
        const result = await exporter.parseAsync(
            modelGroup,
            {
                binary: true
            }
        )

        // GLB should be an ArrayBuffer
        const glbBytes = new Uint8Array(result)

        console.log("GLB size:", glbBytes.length)

        // Ask user where to save
        const filePath = await save({
            defaultPath: "model.glb",
            filters: [
                {
                    name: "GLB Model",
                    extensions: ["glb"]
                }
            ]
        })

        // User cancelled
        if (!filePath) {
            console.log("Export cancelled")
            return
        }

        // Write GLB
        await writeFile(
            filePath,
            glbBytes
        )

        console.log("Model exported:", filePath)

    } catch (error) {

        console.error(
            "GLB export failed:",
            error
        )
    }
}

// -------------------------
// Vue lifecycle
// -------------------------

let unlisten = null
const textureViewer = ref(null)

onMounted(async () => {

    unlisten = await getCurrentWebview().onDragDropEvent(
        async (event) => {

            if (event.payload.type !== "drop") {
                return
            }

            const paths = event.payload.paths

            if (paths.length === 0) {
                return
            }

            const path = paths[0]
            const position = event.payload.position

            console.log("Dropped:", path)
            console.log("Position:", position)

            const lowerPath = path.toLowerCase()

            // -------------------------
            // BML model
            // -------------------------

            // BML
			if (lowerPath.endsWith(".bml")) {
				await LoadModel(path)
				return
			}


			// Texture viewer
			const textureElement = textureViewer.value

			if (textureElement) {
				const rect = textureElement.getBoundingClientRect()

				const insideTextureViewer =
					position.x >= rect.left &&
					position.x <= rect.right &&
					position.y >= rect.top &&
					position.y <= rect.bottom

				if (insideTextureViewer) {
					await ViewTexture(path)
					return
				}
			}


			// Model viewer
			if (!renderer) {
				console.log("Model viewer is not initialized")
				return
			}

			const mesh = getMeshAtPosition(
				position.x,
				position.y
			)

			if (!mesh) {
				console.log("No mesh under drop position")
				return
			}

			await ApplyTexture(path, mesh)
        }
    )
})

onUnmounted(() => {
    if (unlisten) {
        unlisten()
        unlisten = null
    }
})


// DarkModeButton

const isDark = ref(true)

function ToggleDarkMode() {
	isDark.value = !isDark.value
	document.documentElement.classList.toggle('my-app-dark')
}



</script>

<template>
  <div class="app-container">
	
  
    <Tabs v-model:value="activeTab">
      <!-- Top Navigation Tabs -->
      <TabList>
        <Tab value="0">Unpack CPK</Tab>
        <Tab value="1">Unpack PAC</Tab>
		<Tab value="2">View Texture</Tab>
		<Tab value="3">View Model</Tab>
		<Tab value="4">Repack</Tab>
		
		<Button @click="ToggleDarkMode" :label="isDark ? 'Toggle Light Mode' : 'Toggle Dark Mode'" class="DarkModeButton" :icon="isDark ? 'pi pi-sun' : 'pi pi-moon'"/>
		
		
      </TabList>

      <!-- Content Panels -->
      <TabPanels>
        <!-- UNPACK CPK TAB -->
        <TabPanel value="0">
          <div class="tab-content">
            <div class="actions flex items-center gap-2">
				<Button
					label="Open CPK Archive"
					@click="openArchive"
				/>

				<Button
					label="Extract Selected"
					severity="success"
					:disabled="Object.keys(selectedKeys).length === 0"
					@click="extractSelectedFiles"
				/>

				<div class="flex items-center gap-1">
					<Checkbox
						v-model="PacDecompressChecked"
						binary
						inputId="pacDecompression"
					/>
					<label for="pacDecompression" class="cursor-pointer">
						Extract with <br>PAC Decompression
					</label>
				</div>

				
			</div>
			
			<InputText
					v-model="searchText"
					placeholder="Search files..."
					class="searchInputCPK"
			/>

           <div class="content-container">

				<div class="tree-container">
				
					<Button
					size="small"
					@click="toggleSelectAll"
					icon="pi pi-check-square"
					class="select_all_cpk_btn"
					/>
					
					<div class="tree-info">
						<div>Folders: {{ totalsCPK.folders }}
						Files: {{ totalsCPK.files }}</div>
						
						<div>
							Selected: {{ selectedtotalsCPK.folders }} folders,
							{{ selectedtotalsCPK.files }} files
						</div>
					</div>
					
				
					<Tree
						:value="filteredNodes"
						selectionMode="checkbox"
						v-model:selectionKeys="selectedKeys"
						v-model:expandedKeys="expandedKeys"
					>
						<template #empty>
							<p class="empty-msg">No CPK Archives loaded yet.</p>
						</template>
					</Tree>
				</div>

				<div class="table-container">
					<DataTable
						:value="filteredTableRows"
						paginator
						:rows="20"
					>
						<Column field="dir_name" header="Directory" />
						<Column field="file_name" header="Filename" />
						<Column field="file_offset" header="File Offset (relative to TOC)" />
						
						<Column field="file_size" header="File Size">
							<template #body="slotProps">
								{{ formatBytes(slotProps.data.file_size) }}
							</template>
						</Column>

						<Column field="extract_size" header="Extract Size">
							<template #body="slotProps">
								{{ formatBytes(slotProps.data.extract_size) }}
							</template>
						</Column>
						
					</DataTable>
				</div>

			</div>
			
			</div>
			
			<Dialog
				v-model:visible="visibleCPK"
				modal
				header="Extraction"
				:style="{ width: '400px' }"
				:closable="false"
			>
				<p class="ExtractionMessage">
					Extracting files... <br> 
						{{ finished_countCPK }} / {{ filecountCPK }} <br>
						{{ filecountCPK > 0
						? Math.round((finished_countCPK / filecountCPK) * 100)
						: 0 }}%
				</p>

				<Button
					label="Close"
					@click="visibleCPK = false"
				/>
				
				<Button
					label="Stop"
					severity="danger"
					@click="stopExtraction"
					class="StopButtonDialog"
				/>

			</Dialog>
		  
		  
        </TabPanel>


		
		<!-- UNPACK PAC TAB -->
		<TabPanel value="1">
			<div class="tab-content">

				<div class="actions flex items-center gap-2">

					<Button
						label="Open PAC Archive"
						@click="openPACArchive"
					/>

					<Button
						label="Extract Selected"
						severity="success"
						:disabled="Object.keys(selectedKeysPAC).length === 0"
						@click="extractSelectedFilesPAC"
					/>


				</div>

				<InputText
					v-model="searchTextPAC"
					placeholder="Search files..."
					class="searchInputPAC"
				/>

				<div class="content-container">

				<div class="tree-container">
					<Button
					size="small"
					@click="toggleSelectAllPAC"
					icon="pi pi-check-square"
					class="select_all_pac_btn"
					/>
					
					<div class="tree-info">
						<div>Folders: {{ totalsPAC.folders }}
						Files: {{ totalsPAC.files }}</div>
						
						<div>
							Selected: {{ selectedtotalsPAC.folders }} folders,
							{{ selectedtotalsPAC.files }} files
						</div>
					</div>
				
					<Tree
						:value="filteredNodesPAC"
						selectionMode="checkbox"
						v-model:selectionKeys="selectedKeysPAC"
						v-model:expandedKeys="expandedKeysPAC"
					>
						<template #empty>
							<p class="empty-msg">No CPK Archives loaded yet.</p>
						</template>
					</Tree>
				</div>

				<div class="table-container">
					<DataTable
						:value="filteredTableRowsPAC"
						paginator
						:rows="20"
					>
						<Column field="dir_name" header="Directory" />
						<Column field="file_name" header="Filename" />
						<Column field="file_offset" header="File Offset" />
						
						<Column field="file_size" header="File Size">
							<template #body="slotProps">
								{{ formatBytes(slotProps.data.file_size) }}
							</template>
						</Column>

						<Column field="extract_size" header="Extract Size">
							<template #body="slotProps">
								{{ formatBytes(slotProps.data.extract_size) }}
							</template>
						</Column>
						
					</DataTable>
				</div>

			</div>
			</div>
			
			
			<Dialog
				v-model:visible="visiblePAC"
				modal
				header="Extraction"
				:style="{ width: '400px' }"
				:closable="false"
			>
				<p>
					Extracting files... <br> 
						{{ finished_countPAC }} / {{ filecountPAC }} <br>
						{{ filecountPAC > 0
						? Math.round((finished_countPAC / filecountPAC) * 100)
						: 0 }}%
				</p>

				<Button
					label="Close"
					@click="visiblePAC = false"
				/>
				
				<Button
					label="Stop"
					severity="danger"
					@click="stopExtraction"
					class="StopButtonDialog"
				/>

			</Dialog>
			
		</TabPanel>

		
		<!-- VIEW TEXTURE TAB -->
        <TabPanel value="2">
			<div class="tab-content">

				<div class="actions">
					<Button
						label="Select Texture to View"
						@click="ViewTexture()"
						icon="pi pi-image"
					/>

					<Button
						label="Batch Convert to PNG"
						@click="BatchConvertPNG"
					/>
				</div>

				<div ref="textureViewer" class="texture-viewer">
					<img
						v-if="imageSrc"
						:src="imageSrc"
						class="texture-image"
					/>

					<p v-else class="texture-placeholder">
						Drag and drop a texture here
					</p>
				</div>

			</div>
		</TabPanel>
		
		
		
		<!-- VIEW MODEL TAB -->
        <TabPanel value="3">
		  <div class="tab-content">

			<div class="actions">
			  <Button
				label="Select Model to View"
				@click="selectModel"
			  />
			  
			  <Button
				label="Export Model to GLB"
				@click="ExportModel"
			  />
			</div>

			<div
			  id="model-viewer"
			  style="width: 1800px; height: 1200px;"
			></div>

		  </div>
		</TabPanel>
		
		
        <!-- REPACK TAB -->
        <TabPanel value="4">
          <div class="tab-content">
            <div class="actions">
              <Button label="Select Folder to Repack" icon="pi pi-folder-open" />
              <Button label="Build CPK Archive" severity="primary" />
            </div>
            
            <div class="repack-placeholder">
              <p>Select an uncompressed directory to build a new CPK archive.</p>
            </div>
          </div>
        </TabPanel>
		
      </TabPanels>
	  
    </Tabs>
	
  </div>
  
  
</template>

<style scoped>


@font-face {
    font-family: "Roboto";
    src: url("./assets/fonts/Roboto-Regular.ttf") format("truetype");
    font-weight: 400;
    font-style: normal;
}

@font-face {
    font-family: "Pontiac";
    src: url("./assets/fonts/pontiac-regular.otf") format("opentype");
    font-weight: 400;
    font-style: normal;
}

.app-container {
  height: 100vh;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding-top: 1rem;
}

.actions {
  display: flex;
  gap: 0.5rem;
  

}

.content-container {
    display: flex;
    width: 100%;
    gap: 20px;
}

.tree-container {
    width: 35%;
	
}

.table-container {
    width: 65%;
}

.p-tab {
  font-family: "Pontiac", sans-serif;
  
}


.p-tree {
    font-family: "Pontiac", sans-serif;
    font-size: 16px;
}

.p-datatable {
    font-family: "Pontiac", sans-serif;
    font-size: 16px;
}

.p-button {
    font-family: "Pontiac", sans-serif;
    font-size: 16px;
	
	background: var(--app-gradient);
}

.cursor-pointer {
	font-family: "Pontiac", sans-serif;
	font-size: 16px;
}

.searchInputCPK::placeholder {
	font-family: "Pontiac", sans-serif;
	font-size: 14px;
}

.searchInputPAC::placeholder {
	font-family: "Pontiac", sans-serif;
	font-size: 14px;
}

.tree-container .select_all_cpk_btn {
	margin-left: 50px;

}

.tree-container .select_all_pac_btn {
	margin-left: 50px;

}

.empty-msg, .repack-placeholder {
  color: #888;
  margin: 1rem 0;
}

.tree-info  {
	margin-left: 100px;
	transform: translatey(-32px);
	
	font-family: "Pontiac", sans-serif;
	font-size: 14px;
}





.texture-viewer {
    width: 100%;
    min-height: 600px;

    display: flex;
    align-items: center;
    justify-content: center;

    border: 2px dashed #555;
    border-radius: 8px;
}

.texture-image {
    max-width: 100%;
    max-height: 700px;
    object-fit: contain;
}

.texture-placeholder {
    color: #888;
	font-family: "Pontiac", sans-serif;
	font-size: 16px;
}

.repack-placeholder {
	font-family: "Pontiac", sans-serif;
	font-size: 16px;
}

.DarkModeButton {
   margin-left: auto;
   
   width: 150px;
   height: 40px;
}

.p-dialog .p-dialog-header .p-dialog-title {
    font-family: "Pontiac", sans-serif;
}

.StopButtonDialog {
	margin-left: 5px;
}

.ExtractionMessage {
	font-family: "Pontiac", sans-serif;
	font-size: 16px;
}

</style>