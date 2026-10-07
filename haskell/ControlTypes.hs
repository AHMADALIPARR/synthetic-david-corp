module ControlTypes where
import Data.Map.Strict (Map)
import Data.Set (Set)
import Data.Time (UTCTime)

newtype RequestId = RequestId String deriving (Eq, Ord, Show)
newtype WorkId = WorkId String deriving (Eq, Ord, Show)
newtype ProvenanceId = ProvenanceId String deriving (Eq, Ord, Show)
data AgentId = Supervisor | Research | Document | FinancialData | Quant
  | Portfolio | FixedIncome | Equities | Crypto | ESG | LegacyCobol
  | LegacyPLI | Assembler | Batch | LegacyReporting | Database | Ledger
  | Payments | ReverseEngineering | Modernization | MigrationValidation
  | KnowledgeGraph | Reasoning | Provenance | Risk | HumanEscalation
  | Audit | Response | Orchestration | Security deriving (Eq, Ord, Show)
data Permission = LedgerRead | MigrationRead | DocumentRead
  deriving (Eq, Ord, Show)
data TypedPayload = LedgerValidation String | MigrationComparison String
  | DocumentInspection String deriving (Eq, Show)
data ExecutionStatus = Running | Escalated | Completed | Failed | Halted
  deriving (Eq, Show)
data WorkItem = WorkItem
  { wiId :: WorkId, wiParent :: Maybe WorkId, wiAgent :: AgentId
  , wiInput :: TypedPayload, wiDeadline :: UTCTime
  , wiPermissions :: Set Permission } deriving (Show)
data AgentResult = AgentResult
  { arEvidence :: String, arProvenance :: ProvenanceId
  , arAgent :: AgentId, arTool :: String, arInputHash :: String
  , arOutputHash :: String, arTimestamp :: UTCTime
  , arAssumptions :: [String], arUncertainty :: Rational
  , arRisk :: Integer } deriving (Show)
data ExecutionState = ExecutionState
  { esRequestId :: RequestId, esPlan :: [WorkItem]
  , esCompleted :: Map WorkId AgentResult
  , esEvidence :: Map ProvenanceId AgentResult
  , esRiskFlags :: [String], esStatus :: ExecutionStatus } deriving (Show)
