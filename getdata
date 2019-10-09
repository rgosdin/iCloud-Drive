ifndef TREELITE_BASE_H_
define TREELITE_BASE_H_
include <string>
include <unordered_map>
include <cstdint>
namespace treelite {
typedef double tl_float;
enum class SplitFeatureType : int8_t {
kNone, kNumerical, kCategorical
};
enum class Operator : int8_t {
kEQ,  
kLT,  
kLE,  
kGT,  
kGE   
};
extern const std::unordered_map<std::string, Operator> optable;

inline std::string OpName(Operator op) {
switch (op) {
 case Operator::kEQ: return "==";
 case Operator::kLT: return "<";
 case Operator::kLE: return "<=";
 case Operator::kGT: return ">";
 case Operator::kGE: return ">=";
 default: return "";
}
}
 
}  // namespace treelite
 
endif  // TREELITE_BASE_H_