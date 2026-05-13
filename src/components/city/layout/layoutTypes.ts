// Layout type definitions for the 3D city

export interface BuildingLayout {
  nodeId: string        // qualified_name
  x: number            // world-space center X
  z: number            // world-space center Z (Y in 2D layout)
  width: number        // footprint width
  depth: number        // footprint depth
  height: number       // building height
  districtId: number   // parent community id
}

export interface DistrictLayout {
  communityId: number
  name: string
  x: number            // world-space origin X (min corner)
  z: number            // world-space origin Z
  width: number
  depth: number
  color: string
  buildings: BuildingLayout[]
}

export interface CityLayout {
  districts: DistrictLayout[]
  citySize: number
  // Lookup map for fast position queries
  buildingPositions: Map<string, BuildingLayout>
}

export interface RoadSegment {
  from: [number, number, number]   // world XYZ
  to: [number, number, number]
  width: number
  kind: string      // edge kind
  color: string
  confidence: string
  sourceId: string
  targetId: string
}

export interface AggregateRoad {
  from: [number, number, number]
  to: [number, number, number]
  width: number
  laneCount: number
  color: string
  srcCommunity: number
  tgtCommunity: number
  edgeCount: number
}
